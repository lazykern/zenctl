//! Unix-socket client that talks to the native messaging host.
//!
//! Each `zenctl` invocation makes one connection, sends one request,
//! reads one response, then exits. The framing matches the host: 4-byte
//! native-endian length prefix followed by JSON.

use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use zenctl_protocol::{
    Capability, CompactToggleResult, Event, Frame, Method, Request, Response, StatusInfo,
};

/// Path of the unix socket the host listens on.
///
/// Resolution order:
/// 1. `$ZENCTL_SOCKET` env var (explicit override)
/// 2. `$TMPDIR/zenctl.sock` (standard path — works when TMPDIR matches the host's)
/// 3. `/tmp/zenctl.sock` (stable well-known symlink created by the host at startup)
/// 4. Scan `/var/folders/*/*/T/zenctl.sock` (macOS fallback when TMPDIR diverges)
pub fn socket_path() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("ZENCTL_SOCKET") {
        return std::path::PathBuf::from(path);
    }
    let primary = std::env::temp_dir().join("zenctl.sock");
    if primary.exists() {
        return primary;
    }
    let stable = std::path::PathBuf::from("/tmp/zenctl.sock");
    if stable.exists() {
        return stable;
    }
    if let Some(found) = scan_var_folders_socket() {
        return found;
    }
    primary // fall through — connect() will emit a clear error
}

/// Scan macOS `/var/folders/<a>/<b>/T/zenctl.sock` for a socket file.
/// Returns the first match, or `None` if nothing is found.
fn scan_var_folders_socket() -> Option<std::path::PathBuf> {
    let vf = std::path::Path::new("/var/folders");
    let outer = std::fs::read_dir(vf).ok()?;
    for a_entry in outer.flatten() {
        let inner = std::fs::read_dir(a_entry.path()).ok();
        for b_entry in inner.into_iter().flatten().flatten() {
            let candidate = b_entry.path().join("T").join("zenctl.sock");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub struct Client {
    stream: UnixStream,
    next_id: u64,
}

impl Client {
    pub async fn connect() -> Result<Self> {
        let path = socket_path();
        let stream = UnixStream::connect(&path).await.map_err(|e| {
            anyhow!(
                "Could not connect to zenctl host at {}: {e}\n\
                 Make sure Zen Browser is running with the zenctl extension loaded.\n\
                 If this is your first time, run `zenctl install` first.",
                path.display()
            )
        })?;
        Ok(Self { stream, next_id: 1 })
    }

    /// Inject a pre-connected stream — used by tests to supply a socket pair.
    #[cfg(test)]
    pub fn with_stream(stream: UnixStream) -> Self {
        Self { stream, next_id: 1 }
    }

    pub async fn status(&mut self) -> Result<StatusInfo> {
        self.call(Method::Status, serde_json::Value::Null).await
    }

    pub async fn capabilities(&mut self) -> Result<Vec<Capability>> {
        self.call(Method::Capabilities, serde_json::Value::Null)
            .await
    }

    pub async fn compact_toggle(&mut self) -> Result<CompactToggleResult> {
        self.call(Method::CompactToggle, serde_json::Value::Null)
            .await
    }

    pub async fn call_raw(
        &mut self,
        method: Method,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.call_timeout(method, params, None).await
    }

    /// Like `call_raw` but passes a custom timeout to the host so long-running
    /// scripts (e.g. `page eval` automation loops) don't hit the default 15 s cap.
    pub async fn call_raw_timed(
        &mut self,
        method: Method,
        params: serde_json::Value,
        timeout_secs: u64,
    ) -> Result<serde_json::Value> {
        self.call_timeout(method, params, Some(timeout_secs)).await
    }

    /// Send a `Watch` request. The connection then stays open; call
    /// [`recv_event`](Self::recv_event) in a loop to read streamed events.
    pub async fn start_watch(&mut self, topics: &[String]) -> Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Frame::Request(Request {
            id,
            method: Method::Watch,
            params: serde_json::json!({ "topics": topics }),
            timeout_secs: None,
        });
        let buf = serde_json::to_vec(&req)?;
        self.stream
            .write_all(&(buf.len() as u32).to_ne_bytes())
            .await?;
        self.stream.write_all(&buf).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Read the next streamed event. Blocks until one arrives; errors when the
    /// host closes the connection (e.g. the browser quit).
    pub async fn recv_event(&mut self) -> Result<Event> {
        loop {
            let mut len_buf = [0u8; 4];
            self.stream.read_exact(&mut len_buf).await?;
            let len = u32::from_ne_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            self.stream.read_exact(&mut buf).await?;
            if let Frame::Event(ev) = serde_json::from_slice(&buf)? {
                return Ok(ev);
            }
        }
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &mut self,
        method: Method,
        params: serde_json::Value,
    ) -> Result<T> {
        self.call_timeout(method, params, None).await
    }

    async fn call_timeout<T: serde::de::DeserializeOwned>(
        &mut self,
        method: Method,
        params: serde_json::Value,
        timeout_secs: Option<u64>,
    ) -> Result<T> {
        let id = self.next_id;
        self.next_id += 1;

        let req = Frame::Request(Request {
            id,
            method,
            params,
            timeout_secs,
        });
        let buf = serde_json::to_vec(&req)?;
        self.stream
            .write_all(&(buf.len() as u32).to_ne_bytes())
            .await?;
        self.stream.write_all(&buf).await?;

        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_ne_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; len];
        self.stream.read_exact(&mut resp_buf).await?;

        let frame: Frame = serde_json::from_slice(&resp_buf)?;
        if let Frame::Response(Response { data, error, .. }) = frame {
            if let Some(err) = error {
                return Err(anyhow!(err));
            }
            let data = data.unwrap_or(serde_json::Value::Null);
            return Ok(serde_json::from_value(data)?);
        }
        Err(anyhow!("unexpected frame type in response"))
    }
}

#[cfg(test)]
pub mod test_helpers {
    //! Helpers for unit-testing CLI commands with a mock socket-pair transport.
    //!
    //! Usage:
    //! ```ignore
    //! let (client_stream, mut server) = UnixStream::pair().unwrap();
    //! let mut client = Client::with_stream(client_stream);
    //!
    //! // Run CLI command on a background task...
    //! let (method, params) = read_request(&mut server).await;
    //! assert_eq!(method, Method::SplitViewRearrange);
    //! write_response(&mut server, json!({"rearranging": true})).await;
    //! ```

    use super::*;
    use tokio::io::AsyncReadExt;

    /// Read a framed protocol request from the server-side stream.
    /// Returns `(id, method, params, timeout_secs)`.
    pub async fn read_request(
        stream: &mut UnixStream,
    ) -> (u64, Method, serde_json::Value, Option<u64>) {
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .expect("read frame length");
        let len = u32::from_ne_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await.expect("read frame body");
        if let Frame::Request(req) = serde_json::from_slice(&buf).expect("deserialize request") {
            (req.id, req.method, req.params, req.timeout_secs)
        } else {
            panic!("expected Request frame");
        }
    }

    /// Write a framed protocol response to the server-side stream.
    /// `data` is the JSON payload — it will be wrapped in a Response frame.
    pub async fn write_response(stream: &mut UnixStream, data: serde_json::Value) {
        let resp = Frame::Response(Response {
            id: 1,
            data: Some(data),
            error: None,
        });
        let buf = serde_json::to_vec(&resp).expect("serialize response");
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(&(buf.len() as u32).to_ne_bytes())
            .await
            .expect("write frame length");
        stream.write_all(&buf).await.expect("write frame body");
    }

    /// Write an error response to the server-side stream.
    #[allow(dead_code)]
    pub async fn write_error(stream: &mut UnixStream, message: &str) {
        let resp = Frame::Response(Response {
            id: 1,
            data: None,
            error: Some(zenctl_protocol::ProtocolError::Internal {
                message: message.to_string(),
            }),
        });
        let buf = serde_json::to_vec(&resp).expect("serialize error response");
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(&(buf.len() as u32).to_ne_bytes())
            .await
            .expect("write frame length");
        stream.write_all(&buf).await.expect("write frame body");
    }
}
