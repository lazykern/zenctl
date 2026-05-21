//! Native messaging host — runs when the browser spawns `zenctl`.
//!
//! Transport with the extension: 4-byte native-endian length-prefix + JSON
//! on stdin/stdout (the standard Firefox native messaging protocol).
//!
//! Transport with CLI clients: same 4-byte framing over a Unix socket at
//! `$TMPDIR/zenctl.sock`. Each `zenctl` invocation is one connection that
//! sends one request and reads one response.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use zenctl_protocol::{
    Event, Frame, Method, ProtocolError, Request, Response, Tier, PROTOCOL_VERSION,
};

const HOST_VERSION: &str = env!("CARGO_PKG_VERSION");
const EXT_TIMEOUT_SECS: u64 = 15;

pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("ZENCTL_SOCKET") {
        return PathBuf::from(path);
    }
    std::env::temp_dir().join("zenctl.sock")
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

struct State {
    /// Channel to send messages to the extension (via stdout writer task).
    ext_tx: mpsc::UnboundedSender<serde_json::Value>,
    /// Pending CLI requests that are waiting for an extension response.
    pending: HashMap<u64, oneshot::Sender<Response>>,
    next_id: u64,
    /// Fan-out of unsolicited extension events to `zenctl watch` connections.
    events: broadcast::Sender<Event>,
    /// Fingerprint the extension reported in its Hello message. Compared
    /// against `install::extension_fingerprint()` to detect a stale extension.
    loaded_extension_hash: Option<String>,
}

impl State {
    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1);
        id
    }
}

type Shared = Arc<RwLock<State>>;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run() -> Result<()> {
    let sock_path = socket_path();
    // Remove a stale socket from a previous crash.
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;

    // Create a stable well-known symlink at /tmp/zenctl.sock so clients can
    // find the host regardless of which TMPDIR value their process sees.
    // This is critical on macOS where $TMPDIR is per-user and not /tmp.
    // Skip it when the host already binds /tmp/zenctl.sock directly (Linux
    // default TMPDIR): removing + symlinking that path to itself would
    // destroy the live socket and leave a self-referential symlink (ELOOP).
    #[cfg(unix)]
    {
        let well_known = std::path::Path::new("/tmp/zenctl.sock");
        if sock_path.as_path() != well_known {
            let _ = std::fs::remove_file(well_known);
            let _ = std::os::unix::fs::symlink(&sock_path, well_known);
        }
    }

    let (ext_tx, mut ext_rx) = mpsc::unbounded_channel::<serde_json::Value>();
    let ext_ready = Arc::new(AtomicBool::new(false));
    let (events_tx, _) = broadcast::channel::<Event>(256);

    let shared: Shared = Arc::new(RwLock::new(State {
        ext_tx,
        loaded_extension_hash: None,
        pending: HashMap::new(),
        next_id: 1,
        events: events_tx,
    }));

    // Stdout writer: drains the channel and writes native messages to stdout.
    tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(msg) = ext_rx.recv().await {
            if write_native(&mut out, &msg).await.is_err() {
                break;
            }
        }
    });

    // Stdin reader: receives messages from the extension.
    let shared_stdin = shared.clone();
    let ext_ready_stdin = ext_ready.clone();
    let mut stdin_done = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        while let Ok(msg) = read_native(&mut stdin).await {
            on_ext_message(msg, &shared_stdin, &ext_ready_stdin).await;
        }
    });

    // Main loop: accept CLI connections until the extension disconnects.
    loop {
        tokio::select! {
            _ = &mut stdin_done => break,
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let shared = shared.clone();
                        let ready = ext_ready.clone();
                        tokio::spawn(async move {
                            if let Err(e) = serve_cli(stream, shared, ready).await {
                                eprintln!("[zenctl-host] cli error: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("[zenctl-host] accept: {e}");
                        break;
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(&sock_path);
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file("/tmp/zenctl.sock");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Native messaging framing (4-byte native-endian length prefix)
// ---------------------------------------------------------------------------

async fn write_native<W: tokio::io::AsyncWrite + Unpin>(
    w: &mut W,
    msg: &serde_json::Value,
) -> Result<()> {
    let buf = serde_json::to_vec(msg)?;
    w.write_all(&(buf.len() as u32).to_ne_bytes()).await?;
    w.write_all(&buf).await?;
    w.flush().await?;
    Ok(())
}

async fn read_native<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<serde_json::Value> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_ne_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf)?)
}

// ---------------------------------------------------------------------------
// Extension message dispatch
// ---------------------------------------------------------------------------

async fn on_ext_message(msg: serde_json::Value, shared: &Shared, ext_ready: &Arc<AtomicBool>) {
    let frame: Frame = match serde_json::from_value(msg) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[zenctl-host] bad ext message: {e}");
            return;
        }
    };
    match frame {
        Frame::Request(req) if req.method == Method::Hello => {
            ext_ready.store(true, Ordering::Relaxed);
            // Capture the extension's self-reported file fingerprint. A mismatch
            // with the bundled fingerprint surfaces via Status.stale_extension.
            let reported = req
                .params
                .get("extension_fingerprint")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            {
                let mut st = shared.write().await;
                st.loaded_extension_hash = reported.clone();
            }
            if let Some(loaded) = reported.as_deref() {
                let bundled = crate::install::extension_fingerprint();
                if loaded != bundled {
                    eprintln!("[zenctl-host] stale extension: loaded={loaded} bundled={bundled}");
                }
            }
            let ack = serde_json::json!({
                "type": "response",
                "id":   req.id,
                "data": {
                    "protocol_version": PROTOCOL_VERSION,
                    "host_version":     HOST_VERSION,
                    "bundled_extension_hash": crate::install::extension_fingerprint(),
                }
            });
            let _ = shared.read().await.ext_tx.send(ack);
        }
        Frame::Response(resp) => {
            // Extension is replying to a request we forwarded from a CLI client.
            if let Some(tx) = shared.write().await.pending.remove(&resp.id) {
                let _ = tx.send(resp);
            }
        }
        Frame::Event(ev) => {
            // Unsolicited browser event — fan out to `zenctl watch` clients.
            // `send` errors only when there are no subscribers; that is fine.
            let _ = shared.read().await.events.send(ev);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// CLI connection handler
// ---------------------------------------------------------------------------

async fn serve_cli(
    mut stream: UnixStream,
    shared: Shared,
    ext_ready: Arc<AtomicBool>,
) -> Result<()> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_ne_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    let req: Request = serde_json::from_slice(&buf)?;

    // `watch` connections stay open and stream events instead of one response.
    if req.method == Method::Watch {
        return serve_watch(stream, &shared, req).await;
    }

    let resp = dispatch(req, &shared, &ext_ready).await;

    let out = serde_json::to_vec(&Frame::Response(resp))?;
    stream.write_all(&(out.len() as u32).to_ne_bytes()).await?;
    stream.write_all(&out).await?;
    Ok(())
}

/// Serve a long-lived `zenctl watch` connection: subscribe to the event
/// fan-out and stream matching `Frame::Event`s until the client disconnects.
async fn serve_watch(mut stream: UnixStream, shared: &Shared, req: Request) -> Result<()> {
    let topics: Vec<String> = req
        .params
        .get("topics")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut rx = shared.read().await.events.subscribe();
    loop {
        match rx.recv().await {
            Ok(ev) => {
                if !topics.is_empty() && !topics.iter().any(|t| ev.topic.starts_with(t.as_str())) {
                    continue;
                }
                let out = serde_json::to_vec(&Frame::Event(ev))?;
                if stream
                    .write_all(&(out.len() as u32).to_ne_bytes())
                    .await
                    .is_err()
                    || stream.write_all(&out).await.is_err()
                    || stream.flush().await.is_err()
                {
                    break; // client disconnected
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    Ok(())
}

async fn dispatch(req: Request, shared: &Shared, ext_ready: &Arc<AtomicBool>) -> Response {
    match req.method {
        Method::Status => build_status(req.id, shared, ext_ready).await,
        Method::Capabilities => build_capabilities(req.id, shared, ext_ready).await,
        Method::SessionList => handle_session_read(req.id, &req.params).await,
        Method::SessionBackup => handle_session_backup(req.id).await,
        Method::ShortcutsRead => handle_shortcuts_read(req.id).await,
        Method::ShortcutsWrite => handle_shortcuts_write(req.id, &req.params).await,
        m if is_ext_method(m) => {
            match forward(shared, req.id, m, req.params, req.timeout_secs).await {
                Ok(v) => Response::ok(req.id, v),
                Err(e) => Response::err(req.id, e),
            }
        }
        _ => Response::err(req.id, ProtocolError::Unsupported),
    }
}

pub(crate) fn is_ext_method(m: Method) -> bool {
    use Method::*;
    matches!(
        m,
        BookmarksList
            | BookmarksCreate
            | BookmarksUpdate
            | BookmarksRemove
            | TabsList
            | TabsFind
            | TabsOpen
            | TabsClose
            | TabsMove
            | TabsActivate
            | TabsReload
            | TabsDuplicate
            | TabsDiscard
            | TabsSetMuted
            | TabsSetPinned
            | TabsScreenshot
            | TabsZoom
            | TabsReader
            | TabsGoBack
            | TabsGoForward
            | TabGroup
            | TabUngroup
            | SessionsClosed
            | SessionsRestore
            | SessionRestoreWindow
            | SessionRestoreTab
            | DataClear
            | ContainersList
            | ContainersCreate
            | ContainersUpdate
            | ContainersRemove
            | FindInPage
            | FindClear
            | SearchList
            | SearchQuery
            | WindowsList
            | WindowsFocus
            | WindowsClose
            | WindowsCreate
            | WindowsUpdate
            | HistorySearch
            | HistoryDelete
            | HistoryAdd
            | HistoryGetVisits
            | DownloadsList
            | DownloadsCancel
            | DownloadsStart
            | DownloadsPause
            | DownloadsResume
            | CookiesGet
            | CookiesSet
            | CookiesRemove
            | BookmarksMove
            | BookmarksSearch
            | PageInfo
            | PageText
            | PageSource
            | PageSnapshot
            | PageClick
            | PageType
            | PageKey
            | PageWait
            | PageEval
            | PageFrames
            | MediaStatus
            | MediaPlay
            | MediaPause
            | MediaToggle
            | MediaNext
            | MediaPrevious
            | PrefsGet
            | PrefsSet
            | PrefsClear
            | PrefsList
            | CompactToggle
            | CompactSet
            | WorkspaceSwitch
            | WorkspaceList
            | WorkspaceUnload
            | WorkspaceUnloadAll
            | GlanceClose
            | GlanceCloseAll
            | GlanceList
            | GlanceExpand
            | GlanceOpen
            | UrlbarSearch
            | UrlbarClose
            | UrlbarActionsList
            | UrlbarActionsRun
            | EssentialsList
            | EssentialsAdd
            | EssentialsRemove
            | EssentialsReset
            | EssentialsReplaceUrl
            | SplitViewCreate
            | SplitUnsplit
            | SplitViewList
            | SplitViewAddTab
            | SplitViewSetLayout
            | SplitViewResize
            | SplitViewRearrange
            | ShortcutsReset
            | WorkspaceCreate
            | WorkspaceRemove
            | WorkspaceRename
            | WorkspaceSetIcon
            | WorkspaceSetContainer
            | WorkspaceReorder
            | WorkspaceMoveTab
            | CompactHide
            | ModsList
            | ModsInstall
            | ModsRemove
            | ModsEnable
            | ModsDisable
            | ModsPreferences
            | ModsSetPreference
            | FoldersList
            | FoldersCreate
            | FoldersDelete
            | FoldersRename
            | FoldersCollapse
            | FoldersAddTab
            | FoldersSetIcon
            | FoldersCreateSubfolder
            | FoldersUnpack
            | FoldersUnload
            | FoldersMoveToWorkspace
            | FoldersConvertToWorkspace
            | LiveFoldersList
            | LiveFoldersCreate
            | LiveFoldersDelete
            | LiveFoldersRefresh
            | LiveFoldersPause
            | LiveFoldersResume
            | BoostsList
            | BoostsCreate
            | BoostsDelete
            | BoostsActivate
            | BoostsToggle
            | BoostsUpdate
            | WindowSyncForce
            | TabDetach
            | Share
            | ShareCan
            | ExtReload
            | ExtDebug
    )
}

async fn forward(
    shared: &Shared,
    _cli_id: u64,
    method: Method,
    params: serde_json::Value,
    timeout_secs: Option<u64>,
) -> Result<serde_json::Value, ProtocolError> {
    let (ext_id, tx, rx) = {
        let mut s = shared.write().await;
        let tx = s.ext_tx.clone();
        let ext_id = s.alloc_id();
        let (sender, receiver) = oneshot::channel();
        s.pending.insert(ext_id, sender);
        (ext_id, tx, receiver)
    };

    let frame = Frame::Request(Request {
        id: ext_id,
        method,
        params,
        timeout_secs: None, // internal hop; timeout enforced below by the host
    });
    let msg = serde_json::to_value(&frame).map_err(|e| ProtocolError::Internal {
        message: e.to_string(),
    })?;
    if tx.send(msg).is_err() {
        shared.write().await.pending.remove(&ext_id);
        return Err(ProtocolError::ExtensionNotConnected);
    }

    let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(EXT_TIMEOUT_SECS));
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(resp)) => {
            if let Some(err) = resp.error {
                Err(err)
            } else {
                Ok(resp.data.unwrap_or(serde_json::Value::Null))
            }
        }
        Ok(Err(_)) => Err(ProtocolError::ExtensionNotConnected),
        Err(_) => {
            shared.write().await.pending.remove(&ext_id);
            Err(ProtocolError::Internal {
                message: format!(
                    "extension request timed out after {}s",
                    timeout_secs.unwrap_or(EXT_TIMEOUT_SECS)
                ),
            })
        }
    }
}

/// Answer `Capabilities`. The static table from `install.rs` lists every
/// method optimistically; when the extension is connected we probe it for
/// live experiment-API availability and downgrade preference/UI-automation
/// capabilities that depend on an experiment that is not loaded.
async fn build_capabilities(id: u64, shared: &Shared, ext_ready: &Arc<AtomicBool>) -> Response {
    let mut caps = crate::install::capabilities();
    if ext_ready.load(Ordering::Relaxed) {
        if let Ok(probe) = forward(
            shared,
            id,
            Method::CapabilitiesProbe,
            serde_json::Value::Null,
            None,
        )
        .await
        {
            let zen_chrome = probe
                .get("zen_chrome")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let zen_prefs = probe
                .get("zen_prefs")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            for cap in &mut caps {
                let ok = match cap.tier {
                    Tier::Preference => zen_prefs,
                    Tier::UiAutomation => zen_chrome,
                    _ => true,
                };
                if !ok {
                    cap.available = false;
                    cap.reason = Some(
                        "experiment API unavailable — enable \
                         extensions.experiments.enabled in about:config"
                            .into(),
                    );
                }
            }
        }
    }
    match serde_json::to_value(&caps) {
        Ok(v) => Response::ok(id, v),
        Err(e) => Response::err(
            id,
            ProtocolError::Internal {
                message: e.to_string(),
            },
        ),
    }
}

async fn build_status(id: u64, shared: &Shared, ext_ready: &Arc<AtomicBool>) -> Response {
    let extension_connected = ext_ready.load(Ordering::Relaxed);
    let profile_path = crate::profile::detect_profile().ok().flatten();
    let zen_running = profile_path.as_ref().map_or_else(
        || crate::profile::zen_pid().is_some(),
        |p| crate::profile::is_running(p),
    );
    let zen_pid = if zen_running {
        crate::profile::zen_pid()
    } else {
        None
    };

    #[cfg(target_os = "macos")]
    let window_count = if zen_running {
        tokio::task::spawn_blocking(crate::zen_macos::count_windows)
            .await
            .ok()
            .and_then(|r| r.ok())
    } else {
        None
    };
    #[cfg(not(target_os = "macos"))]
    let window_count: Option<u32> = None;

    let bundled = crate::install::extension_fingerprint().to_string();
    let loaded = shared.read().await.loaded_extension_hash.clone();
    let stale_extension = match (extension_connected, &loaded) {
        (true, Some(h)) => h.as_str() != bundled,
        _ => false,
    };

    let info = zenctl_protocol::StatusInfo {
        daemon_version: HOST_VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION,
        extension_connected,
        zen_running,
        profile_path: profile_path.map(|p| p.display().to_string()),
        zen_pid,
        window_count,
        stale_extension,
        bundled_extension_hash: Some(bundled),
        loaded_extension_hash: loaded,
    };
    match serde_json::to_value(&info) {
        Ok(v) => Response::ok(id, v),
        Err(e) => Response::err(
            id,
            ProtocolError::Internal {
                message: e.to_string(),
            },
        ),
    }
}

async fn handle_session_read(id: u64, params: &serde_json::Value) -> Response {
    let as_tab_list = params
        .get("tab_list")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    match crate::session::read_current_session() {
        Ok(Some(session)) => {
            if as_tab_list {
                let tabs = crate::session::tab_list(&session);
                match serde_json::to_value(&tabs) {
                    Ok(v) => Response::ok(id, v),
                    Err(e) => Response::err(
                        id,
                        ProtocolError::Internal {
                            message: e.to_string(),
                        },
                    ),
                }
            } else {
                Response::ok(id, session)
            }
        }
        Ok(None) => Response::err(
            id,
            ProtocolError::ProfileParseError {
                message: "no sessionstore file found".into(),
            },
        ),
        Err(e) => {
            let msg = format!("{e:#?}");
            let variant =
                if msg.contains("permission") || msg.contains("locked") || msg.contains("access") {
                    ProtocolError::ProfileLocked { message: msg }
                } else {
                    ProtocolError::ProfileParseError { message: msg }
                };
            Response::err(id, variant)
        }
    }
}

async fn handle_shortcuts_read(id: u64) -> Response {
    match crate::shortcuts::read_shortcuts() {
        Ok(Some(data)) => Response::ok(id, data),
        Ok(None) => Response::err(
            id,
            ProtocolError::ProfileParseError {
                message: "no zen-keyboard-shortcuts.json found".into(),
            },
        ),
        Err(e) => {
            let msg = format!("{e:#?}");
            let variant =
                if msg.contains("permission") || msg.contains("locked") || msg.contains("access") {
                    ProtocolError::ProfileLocked { message: msg }
                } else {
                    ProtocolError::ProfileParseError { message: msg }
                };
            Response::err(id, variant)
        }
    }
}

async fn handle_shortcuts_write(id: u64, params: &serde_json::Value) -> Response {
    let data = params.get("shortcuts").unwrap_or(params);
    match crate::shortcuts::write_shortcuts(data) {
        Ok(path) => Response::ok(id, serde_json::json!({ "written": path.to_string_lossy() })),
        Err(e) => {
            let msg = format!("{e:#?}");
            let variant =
                if msg.contains("permission") || msg.contains("locked") || msg.contains("access") {
                    ProtocolError::ProfileLocked { message: msg }
                } else {
                    ProtocolError::ProfileParseError { message: msg }
                };
            Response::err(id, variant)
        }
    }
}

async fn handle_session_backup(id: u64) -> Response {
    match crate::session::backup_sessionstore() {
        Ok(Some(path)) => Response::ok(id, serde_json::json!({ "backup": path.to_string_lossy() })),
        Ok(None) => Response::err(
            id,
            ProtocolError::ProfileParseError {
                message: "no sessionstore file to back up".into(),
            },
        ),
        Err(e) => {
            let msg = format!("{e:#?}");
            let variant =
                if msg.contains("permission") || msg.contains("locked") || msg.contains("access") {
                    ProtocolError::ProfileLocked { message: msg }
                } else {
                    ProtocolError::ProfileParseError { message: msg }
                };
            Response::err(id, variant)
        }
    }
}
