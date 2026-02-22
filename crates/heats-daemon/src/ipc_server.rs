use std::sync::{Arc, Mutex};

use iced::futures::SinkExt;
use iced::Subscription;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

use crate::app::{Message, ResponseSender};
use heats_core::source::{DmenuItem, SourceItem};

/// Index mapping from filtered display order to original raw_lines position.
/// When JSONL lines fail to parse, the SourceItem list is shorter than raw_lines,
/// so we need to track which raw_lines index each SourceItem came from.

/// IPC context sent as the first line by the client
#[derive(serde::Deserialize)]
struct IpcContext {
    format: String,
}

/// Create an iced Subscription that listens on the Unix domain socket.
/// Accepts one connection at a time: reads line-delimited items, then sends
/// a `Message::DmenuSession` containing the items and a oneshot channel for the response.
pub fn dmenu_subscription() -> Subscription<Message> {
    Subscription::run(dmenu_stream)
}

fn dmenu_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(
        4,
        |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            let sock_path = heats_core::ipc::socket_path();

            // Remove stale socket (in case daemon didn't clean up)
            let _ = std::fs::remove_file(&sock_path);

            let listener = match UnixListener::bind(&sock_path) {
                Ok(l) => {
                    tracing::info!("IPC listening on {}", sock_path.display());
                    l
                }
                Err(e) => {
                    tracing::error!("Failed to bind IPC socket {}: {}", sock_path.display(), e);
                    // Keep the subscription alive but idle
                    std::future::pending::<()>().await;
                    unreachable!()
                }
            };

            loop {
                let (stream, _addr) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        tracing::error!("IPC accept error: {}", e);
                        continue;
                    }
                };

                tracing::debug!("IPC client connected");

                let mut reader = BufReader::new(stream);

                // Read the first line as context
                let mut first_line = String::new();
                match reader.read_line(&mut first_line).await {
                    Ok(0) => {
                        tracing::debug!("IPC client disconnected immediately");
                        continue;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("IPC read error on context line: {}", e);
                        continue;
                    }
                }

                let first_line = first_line.trim().to_string();

                // Try to parse as IPC context
                let (format, remaining_first_line) =
                    match serde_json::from_str::<IpcContext>(&first_line) {
                        Ok(ctx) => (ctx.format, None),
                        Err(_) => {
                            // Not a context line — treat as legacy text format
                            // The first line is actually an item
                            ("text".to_string(), Some(first_line))
                        }
                    };

                let is_jsonl = format == "jsonl";

                // Read remaining lines
                let mut raw_lines = Vec::new();
                if let Some(line) = remaining_first_line {
                    if !line.is_empty() {
                        raw_lines.push(line);
                    }
                }

                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
                            if !trimmed.is_empty() {
                                raw_lines.push(trimmed.to_string());
                            }
                        }
                        Err(e) => {
                            tracing::error!("IPC read error: {}", e);
                            break;
                        }
                    }
                }

                if raw_lines.is_empty() {
                    tracing::debug!("IPC client sent no items, ignoring");
                    continue;
                }

                tracing::info!(
                    "IPC received {} items (format: {})",
                    raw_lines.len(),
                    format
                );

                // Convert to SourceItems based on format.
                // index_map tracks: items[i] came from raw_lines[index_map[i]].
                let (items, index_map): (Vec<SourceItem>, Vec<usize>) = if is_jsonl {
                    raw_lines
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, line)| {
                            match serde_json::from_str::<DmenuItem>(line) {
                                Ok(di) => {
                                    let si = SourceItem {
                                        title: di.title.clone(),
                                        subtitle: di.subtitle.clone(),
                                        exec_path: di.get_field("data"),
                                        source_name: "dmenu".to_string(),
                                        icon: None,
                                    };
                                    Some((si, idx))
                                }
                                Err(e) => {
                                    tracing::debug!("Failed to parse JSONL line: {}", e);
                                    None
                                }
                            }
                        })
                        .unzip()
                } else {
                    raw_lines
                        .iter()
                        .enumerate()
                        .map(|(idx, title)| {
                            let si = SourceItem {
                                title: title.clone(),
                                subtitle: None,
                                exec_path: String::new(),
                                source_name: "dmenu".to_string(),
                                icon: None,
                            };
                            (si, idx)
                        })
                        .unzip()
                };

                // Create a oneshot channel for the response (selected index)
                let (response_tx, response_rx) = oneshot::channel::<Option<usize>>();

                // Wrap sender in Arc<Mutex<Option<...>>> so Message can be Clone
                let wrapped_tx = ResponseSender(Arc::new(Mutex::new(Some(response_tx))));

                // Send the session to the iced app
                let msg = Message::DmenuSession {
                    items,
                    response_tx: wrapped_tx,
                };
                if sender.send(msg).await.is_err() {
                    tracing::error!("Failed to send DmenuSession to app");
                    continue;
                }

                // Wait for the app to send back a response (selected index),
                // then write the corresponding raw line to the client
                let stream = reader.into_inner();
                match response_rx.await {
                    Ok(Some(selected_index)) => {
                        // Map the selected_index (from filtered items) back to
                        // the original raw_lines position
                        let response = index_map
                            .get(selected_index)
                            .and_then(|&raw_idx| raw_lines.get(raw_idx))
                            .cloned()
                            .unwrap_or_default();

                        let mut writer = stream;
                        let payload = format!("{response}\n");
                        if let Err(e) = writer.write_all(payload.as_bytes()).await {
                            tracing::error!("IPC write error: {}", e);
                        }
                        let _ = writer.shutdown().await;
                    }
                    Ok(None) | Err(_) => {
                        // Cancelled or channel dropped — just close
                        let mut writer = stream;
                        let _ = writer.shutdown().await;
                    }
                }
            }
        },
    )
}
