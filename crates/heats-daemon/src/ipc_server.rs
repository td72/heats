use std::sync::{Arc, Mutex};

use iced::futures::SinkExt;
use iced::Subscription;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

use crate::app::{Message, ResponseSender};
use heats_core::source::{DmenuItem, SourceItem};

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

                // Convert to SourceItems based on format
                let (items, dmenu_items): (Vec<SourceItem>, Vec<Option<DmenuItem>>) = if is_jsonl {
                    raw_lines
                        .iter()
                        .filter_map(|line| {
                            match serde_json::from_str::<DmenuItem>(line) {
                                Ok(di) => {
                                    let si = SourceItem {
                                        title: di.title.clone(),
                                        subtitle: di.subtitle.clone(),
                                        exec_path: di.get_field("data"),
                                        source_name: "dmenu".to_string(),
                                        icon: None, // No icon loading for IPC items
                                    };
                                    Some((si, Some(di)))
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
                        .map(|title| {
                            let si = SourceItem {
                                title: title.clone(),
                                subtitle: None,
                                exec_path: String::new(),
                                source_name: "dmenu".to_string(),
                                icon: None,
                            };
                            (si, None)
                        })
                        .unzip()
                };

                // Create a oneshot channel for the response
                let (response_tx, response_rx) = oneshot::channel::<Option<String>>();

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

                // Wait for the app to send back a response, then write to the client
                let stream = reader.into_inner();
                match response_rx.await {
                    Ok(Some(selected)) => {
                        // For JSONL format, try to return the data field
                        let response = if is_jsonl {
                            // Find the DmenuItem whose title matches the selected
                            dmenu_items
                                .iter()
                                .flatten()
                                .find(|di| di.title == selected)
                                .map(|di| di.get_field("data"))
                                .unwrap_or(selected)
                        } else {
                            selected
                        };

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
