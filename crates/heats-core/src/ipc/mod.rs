use std::path::PathBuf;

/// Resolve the runtime directory for IPC files.
/// Uses $XDG_RUNTIME_DIR, falling back to /tmp/heats-{uid}.
/// Creates the directory if it does not exist.
fn runtime_dir() -> PathBuf {
    let dir = if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir)
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/xdg-runtime-{uid}"))
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Resolve the Unix domain socket path for IPC.
pub fn socket_path() -> PathBuf {
    runtime_dir().join("heats.sock")
}

/// Resolve the PID file path for daemon management.
pub fn pid_path() -> PathBuf {
    runtime_dir().join("heats.pid")
}

/// Write the current process PID to the PID file.
pub fn write_pid() {
    let path = pid_path();
    if let Err(e) = std::fs::write(&path, std::process::id().to_string()) {
        tracing::warn!("Failed to write PID file {}: {}", path.display(), e);
    }
}

/// Read the PID from the PID file. Returns None if the file doesn't exist or is invalid.
pub fn read_pid() -> Option<u32> {
    let path = pid_path();
    std::fs::read_to_string(&path)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

/// Remove the PID file.
pub fn remove_pid() {
    let path = pid_path();
    let _ = std::fs::remove_file(&path);
}
