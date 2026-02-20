use std::sync::{Arc, Mutex};

use iced::futures::SinkExt;
use iced::Subscription;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

use crate::app::{Message, ResponseSender};

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
            let sock_path = super::socket_path();

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
                let mut items = Vec::new();

                // Read lines until EOF
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
                            if !trimmed.is_empty() {
                                items.push(trimmed.to_string());
                            }
                        }
                        Err(e) => {
                            tracing::error!("IPC read error: {}", e);
                            break;
                        }
                    }
                }

                if items.is_empty() {
                    tracing::debug!("IPC client sent no items, ignoring");
                    continue;
                }

                tracing::info!("IPC received {} items", items.len());

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
                        let mut writer = stream;
                        let payload = format!("{selected}\n");
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
