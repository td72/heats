use std::io::{self, BufRead};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// IPC format for communication with daemon
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IpcFormat {
    Text,
    Jsonl,
}

/// Read items from stdin, send them to the daemon, and return the selected item.
/// Returns `Ok(Some(selected))` if user selected, `Ok(None)` if cancelled,
/// and `Err` if the daemon is unreachable or an I/O error occurs.
pub async fn send_and_receive(
    items: Vec<String>,
    format: IpcFormat,
) -> io::Result<Option<String>> {
    let sock_path = heats_core::ipc::socket_path();

    let stream = UnixStream::connect(&sock_path).await.map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("heatsd is not running ({})", sock_path.display()),
        )
    })?;

    let (reader, mut writer) = stream.into_split();

    // Send context line
    let context = match format {
        IpcFormat::Text => r#"{"format":"text"}"#,
        IpcFormat::Jsonl => r#"{"format":"jsonl"}"#,
    };
    writer.write_all(context.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    // Send items as newline-delimited text
    for item in &items {
        writer.write_all(item.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }
    // Signal end of items
    writer.shutdown().await?;

    // Read response (selected item or empty = cancelled)
    let mut buf_reader = BufReader::new(reader);
    let mut response = String::new();
    buf_reader.read_line(&mut response).await?;

    let trimmed = response.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

/// Read all lines from stdin (blocking).
pub fn read_stdin_items() -> Vec<String> {
    let stdin = io::stdin();
    stdin
        .lock()
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            if line.is_empty() {
                None
            } else {
                Some(line)
            }
        })
        .collect()
}
