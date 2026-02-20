pub mod client;
pub mod server;

use std::path::PathBuf;

/// Resolve the Unix domain socket path for IPC.
/// Uses $XDG_RUNTIME_DIR/heats.sock, falling back to /tmp/heats-{uid}.sock.
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("heats.sock");
    }
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/heats-{uid}.sock"))
}
