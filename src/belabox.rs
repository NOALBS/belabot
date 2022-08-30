use std::sync::{Arc, Weak};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot, Mutex},
    task::JoinHandle,
    time::{self, Duration},
};
use tokio_tungstenite::{
    tungstenite::{self, protocol::CloseFrame, Message as TMessage},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, trace, warn};

pub mod messages;
pub mod requests;

pub use messages::Message;
pub use requests::Request;

pub type Writer = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, TMessage>;
pub type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

const BELABOX_WS: &str = "wss://remote.belabox.net/ws/remote";

#[derive(Error, Debug)]
pub enum BelaboxError {
    #[error("websocket error")]
    Connect(#[source] tungstenite::Error),
    #[error("websocket send error")]
    Send(#[source] tungstenite::Error),
    #[error("disconnected from BELABOX Cloud")]
    Disconnected,
    #[error("auth failed")]
    AuthFailed,
    #[error("Receiver closed")]
    ReceiverClosed(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Already restarting")]
    AlreadyRestarting,
}

pub struct Belabox {
    pub run_handle: JoinHandle<()>,
    pub message_tx: Weak<broadcast::Sender<Message>>,
    write: mpsc::UnboundedSender<InnerMessage>,
}

#[derive(Debug)]
struct InnerMessage {
    pub respond: oneshot::Sender<Result<(), BelaboxError>>,
    pub message: String,
}

impl Belabox {
    pub async fn connect(key: String) -> Result<Self, BelaboxError> {
        let (inner_tx, inner_rx) = mpsc::unbounded_channel();
        let (message_tx, _) = broadcast::channel(100);
        let message_tx = Arc::new(message_tx);

        let auth = requests::Remote::AuthKey { key, version: 6 };
        let run_handle = tokio::spawn(run_loop(auth, message_tx.clone(), inner_rx));

        Ok(Self {
            run_handle,
            message_tx: Arc::downgrade(&message_tx),
            write: inner_tx,
        })
    }

    pub fn message_stream(&self) -> Result<broadcast::Receiver<Message>, BelaboxError> {
        let tx = self
            .message_tx
            .upgrade()
            .ok_or(BelaboxError::Disconnected)?;

        Ok(tx.subscribe())
    }

    pub async fn send(&self, request: Request) -> Result<(), BelaboxError> {
        let message = serde_json::to_string(&request).unwrap();
        let (tx, rx) = oneshot::channel();
        let inner = InnerMessage {
            respond: tx,
            message,
        };

        self.write.send(inner).unwrap();

        rx.await.map_err(BelaboxError::ReceiverClosed)?
    }

    pub async fn start(&self, start: requests::Start) -> Result<(), BelaboxError> {
        let request = Request::Start(start);

        self.send(request).await
    }

    pub async fn stop(&self) -> Result<(), BelaboxError> {
        let request = Request::Stop(None);

        self.send(request).await
    }

    pub async fn command(&self, command: requests::Command) -> Result<(), BelaboxError> {
        let request = Request::Command(command);

        self.send(request).await
    }

    pub async fn restart(&self) -> Result<(), BelaboxError> {
        self.command(requests::Command::Reboot).await
    }

    pub async fn poweroff(&self) -> Result<(), BelaboxError> {
        self.command(requests::Command::Poweroff).await
    }

    pub async fn bitrate(&self, max_br: u32) -> Result<(), BelaboxError> {
        let request = Request::Bitrate(requests::Bitrate { max_br });

        self.send(request).await
    }

    pub async fn netif(&self, network: requests::Netif) -> Result<(), BelaboxError> {
        let request = Request::Netif(network);

        self.send(request).await
    }
}

async fn run_loop(
    auth: requests::Remote,
    message_tx: Arc<broadcast::Sender<Message>>,
    inner_rx: mpsc::UnboundedReceiver<InnerMessage>,
) {
    // Spawn thread to handle inner requests
    let request_write = Arc::new(Mutex::new(None));
    tokio::spawn(handle_requests(inner_rx, request_write.clone()));

    loop {
        let ws_stream = get_connection().await;
        let (mut write, read) = ws_stream.split();

        // Authenticate
        let auth_request = serde_json::to_string(&Request::Remote(auth.clone())).unwrap();
        if let Err(e) = write.send(TMessage::Text(auth_request)).await {
            error!(?e, "error sending auth message");
            continue;
        };

        {
            *request_write.lock().await = Some(write);
        }

        // Spawn thread to handle keepalive
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(keepalive(request_write.clone(), cancel_rx));

        // Handle messages
        if let Err(BelaboxError::AuthFailed) = handle_messages(read, message_tx.clone()).await {
            break;
        };

        // Disconnected
        let _ = cancel_tx.send(());

        {
            *request_write.lock().await = None;
        }
    }
}

async fn get_connection() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let mut retry_grow = 1;

    loop {
        info!("Connecting");

        if let Ok((ws_stream, _)) = tokio_tungstenite::connect_async(BELABOX_WS).await {
            info!("Connected");
            break ws_stream;
        }

        let wait = 1 << retry_grow;
        warn!("Unable to connect");
        info!("trying to connect again in {} seconds", wait);
        tokio::time::sleep(Duration::from_secs(wait)).await;

        if retry_grow < 5 {
            retry_grow += 1;
        }
    }
}

async fn keepalive(write: Arc<Mutex<Option<Writer>>>, mut cancel_rx: oneshot::Receiver<()>) {
    loop {
        time::sleep(Duration::from_secs(5)).await;

        if cancel_rx.try_recv().is_ok() {
            debug!("keepalive cancel received");
            break;
        }

        debug!("Sending keepalive");

        if let Some(w) = write.lock().await.as_mut() {
            if (w
                .send(TMessage::Text(
                    serde_json::to_string(&Request::Keepalive(None)).unwrap(),
                ))
                .await)
                .is_err()
            {
                break;
            }
        }
    }

    debug!("Keepalive stopped")
}

async fn handle_messages(
    mut read: Reader,
    message_tx: Arc<broadcast::Sender<Message>>,
) -> Result<(), BelaboxError> {
    while let Some(Ok(message)) = read.next().await {
        if let TMessage::Close(info) = &message {
            if let Some(CloseFrame { reason, .. }) = info {
                info!(%reason, "connection closed with reason");
            }

            continue;
        }

        if let TMessage::Text(text) = &message {
            let text: serde_json::Value = match serde_json::from_str(text) {
                Ok(o) => o,
                Err(e) => {
                    error!(?e, text, "failed to deserialize");
                    continue;
                }
            };

            let text = match text.as_object() {
                Some(o) => o,
                None => {
                    error!(?text, "not an object");
                    continue;
                }
            };

            for (key, value) in text {
                let m: Message = match serde_json::from_value(value.to_owned()) {
                    Ok(o) => o,
                    Err(e) => {
                        error!(?e, ?key, ?value, "failed to deserialize");
                        continue;
                    }
                };

                if let Message::RemoteAuth(remote) = &m {
                    if !remote.auth_key {
                        error!("Failed to authenticate");
                        return Err(BelaboxError::AuthFailed);
                    }
                }

                trace!(?m, "Received message");
                let _ = message_tx.send(m);
            }
        }
    }

    warn!("Disconnected from BELABOX Cloud");

    Ok(())
}

// TODO: Add retry or timeout?
async fn handle_requests(
    mut inner_rx: mpsc::UnboundedReceiver<InnerMessage>,
    write: Arc<Mutex<Option<Writer>>>,
) {
    while let Some(request) = inner_rx.recv().await {
        trace!(?request.message, "sending");

        let mut lock = write.lock().await;
        if let Some(w) = lock.as_mut() {
            let res = w
                .send(TMessage::Text(request.message))
                .await
                .map_err(BelaboxError::Send);

            request.respond.send(res).unwrap();
        } else {
            request
                .respond
                .send(Err(BelaboxError::Disconnected))
                .unwrap();
        }
    }
}
