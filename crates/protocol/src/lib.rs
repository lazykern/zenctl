//! Wire protocol shared between the zenctl daemon, client SDK, and WebExtension.
//!
//! Transport is JSON over WebSocket on `127.0.0.1`. Every message is a
//! [`Frame`] which is either a [`Request`], a [`Response`], or an
//! unsolicited [`Event`]. Request/response pairs are correlated by `id`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Request(Request),
    Response(Response),
    Event(Event),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: Method,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
    /// Per-request timeout override in seconds. When absent the host uses its
    /// default (15 s). Useful for long-running `page eval` automation scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

impl Response {
    pub fn ok(id: u64, data: serde_json::Value) -> Self {
        Self {
            id,
            data: Some(data),
            error: None,
        }
    }
    pub fn err(id: u64, error: ProtocolError) -> Self {
        Self {
            id,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    // Meta
    Hello,
    Status,
    Capabilities,
    ExtReload,
    ExtDebug,
    Watch,

    // Stable-API tier (extension-proxied)
    BookmarksList,
    BookmarksCreate,
    BookmarksUpdate,
    BookmarksRemove,

    TabsList,
    TabsFind,
    TabsOpen,
    TabsClose,
    TabsMove,
    TabsActivate,
    TabsReload,
    TabsDuplicate,
    TabsDiscard,
    TabsSetMuted,
    TabsSetPinned,
    TabsScreenshot,
    TabsZoom,
    TabsReader,
    TabsGoBack,
    TabsGoForward,
    TabGroup,
    TabUngroup,

    WindowsList,
    WindowsFocus,
    WindowsClose,
    WindowsCreate,
    WindowsUpdate,

    HistorySearch,
    HistoryDelete,
    HistoryAdd,
    HistoryGetVisits,

    DownloadsList,
    DownloadsCancel,
    DownloadsStart,
    DownloadsPause,
    DownloadsResume,

    CookiesGet,
    CookiesSet,
    CookiesRemove,

    BookmarksMove,
    BookmarksSearch,

    SessionsClosed,
    SessionsRestore,
    SessionRestoreWindow,
    SessionRestoreTab,

    DataClear,

    ContainersList,
    ContainersCreate,
    ContainersUpdate,
    ContainersRemove,

    FindInPage,
    FindClear,

    SearchList,
    SearchQuery,

    // Page interaction tier (extension-injected content scripts)
    PageInfo,
    PageText,
    PageSource,
    PageSnapshot,
    PageClick,
    PageType,
    PageKey,
    PageWait,
    PageEval,
    PageFrames,

    // Page media tier
    MediaStatus,
    MediaPlay,
    MediaPause,
    MediaToggle,
    MediaNext,
    MediaPrevious,

    // Preference tier
    PrefsGet,
    PrefsSet,
    PrefsClear,
    PrefsList,

    // Profile-file tier
    SessionList,
    SessionBackup,
    ShortcutsRead,
    ShortcutsWrite,

    // UI-automation tier
    WorkspaceList,
    WorkspaceSwitch,
    WorkspaceUnload,
    WorkspaceUnloadAll,
    WorkspaceCreate,
    WorkspaceRemove,
    WorkspaceRename,
    WorkspaceSetIcon,
    WorkspaceSetContainer,
    WorkspaceReorder,
    WorkspaceMoveTab,
    CompactToggle,
    CompactSet,
    CompactHide,
    SplitViewCreate,
    SplitUnsplit,
    SplitViewList,
    SplitViewAddTab,
    SplitViewSetLayout,
    SplitViewResize,
    SplitViewRearrange,
    ShortcutsReset,
    GlanceClose,
    GlanceCloseAll,
    GlanceList,
    GlanceExpand,
    GlanceOpen,

    UrlbarSearch,
    UrlbarClose,
    UrlbarActionsList,
    UrlbarActionsRun,

    EssentialsList,
    EssentialsAdd,
    EssentialsRemove,
    EssentialsReset,
    EssentialsReplaceUrl,

    Share,
    ShareCan,

    ModsList,
    ModsInstall,
    ModsRemove,
    ModsEnable,
    ModsDisable,
    ModsPreferences,
    ModsSetPreference,

    FoldersList,
    FoldersCreate,
    FoldersDelete,
    FoldersRename,
    FoldersCollapse,
    FoldersAddTab,
    FoldersSetIcon,
    FoldersCreateSubfolder,
    FoldersUnpack,
    FoldersUnload,
    FoldersMoveToWorkspace,
    FoldersConvertToWorkspace,

    LiveFoldersList,
    LiveFoldersCreate,
    LiveFoldersDelete,
    LiveFoldersRefresh,
    LiveFoldersPause,
    LiveFoldersResume,

    BoostsList,
    BoostsCreate,
    BoostsDelete,
    BoostsActivate,
    BoostsToggle,
    BoostsUpdate,

    WindowSyncForce,

    TabDetach,

    /// Internal: host probes the extension for live experiment-API
    /// availability when answering `Capabilities`. Never sent by the CLI.
    CapabilitiesProbe,
}

/// Support tier for a capability — informs the caller how reliable it is
/// and what preconditions (e.g. browser closed) apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    StableApi,
    Preference,
    ProfileFile,
    UiAutomation,
    Experimental,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub method: Method,
    pub tier: Tier,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub daemon_version: String,
    pub protocol_version: u32,
    pub extension_connected: bool,
    pub zen_running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zen_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_count: Option<u32>,
    /// True when the extension's reported fingerprint doesn't match the
    /// fingerprint of the files bundled into this host binary — i.e. Zen has
    /// loaded a stale extension. CLI prints a warning when this is set.
    #[serde(default)]
    pub stale_extension: bool,
    /// SHA-256 of the extension files bundled into the host binary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundled_extension_hash: Option<String>,
    /// SHA-256 the connected extension reported at Hello.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_extension_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactToggleResult {
    pub triggered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Result of a single pref read/write. `value` is one of bool/int/string —
/// the daemon keeps it as a raw JSON value rather than a tagged enum so the
/// CLI can pretty-print it directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefEntry {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub has_user_value: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ProtocolError {
    #[error("daemon not running")]
    DaemonNotRunning,
    #[error("extension not connected")]
    ExtensionNotConnected,
    #[error("zen not running")]
    ZenNotRunning,
    #[error("profile locked: {message}")]
    ProfileLocked { message: String },
    #[error("profile parse error: {message}")]
    ProfileParseError { message: String },
    #[error("unsupported method")]
    Unsupported,
    #[error("auth failed")]
    AuthFailed,
    #[error("invalid params: {message}")]
    InvalidParams { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(frame: &Frame) -> Frame {
        let json = serde_json::to_string(frame).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn request_frame_roundtrips() {
        let frame = Frame::Request(Request {
            id: 7,
            method: Method::TabsScreenshot,
            params: serde_json::json!({ "target": { "active": true } }),
            timeout_secs: None,
        });
        match roundtrip(&frame) {
            Frame::Request(r) => {
                assert_eq!(r.id, 7);
                assert_eq!(r.method, Method::TabsScreenshot);
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn response_frames_roundtrip() {
        let ok = roundtrip(&Frame::Response(Response::ok(
            1,
            serde_json::json!({ "x": 1 }),
        )));
        match ok {
            Frame::Response(r) => {
                assert_eq!(r.id, 1);
                assert!(r.error.is_none());
                assert!(r.data.is_some());
            }
            other => panic!("expected response, got {other:?}"),
        }
        let err = roundtrip(&Frame::Response(Response::err(
            2,
            ProtocolError::Unsupported,
        )));
        match err {
            Frame::Response(r) => {
                assert_eq!(r.id, 2);
                assert!(matches!(r.error, Some(ProtocolError::Unsupported)));
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn event_frame_roundtrips() {
        let frame = Frame::Event(Event {
            topic: "tabs.updated".into(),
            payload: serde_json::json!({ "tab_id": 3 }),
        });
        match roundtrip(&frame) {
            Frame::Event(e) => assert_eq!(e.topic, "tabs.updated"),
            other => panic!("expected event, got {other:?}"),
        }
    }

    #[test]
    fn method_serializes_snake_case() {
        let json = serde_json::to_string(&Method::SessionsRestore).expect("serialize");
        assert_eq!(json, "\"sessions_restore\"");
    }
}
