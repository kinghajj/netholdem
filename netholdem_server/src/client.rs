use std::error::Error;
use std::net::SocketAddr;

use futures::task::Context;
use futures::{SinkExt, Stream};
use log::{debug, error, info};
use snafu::Snafu;
use tokio;
use tokio::macros::support::{Pin, Poll};
use tokio::net::TcpStream;
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio::task::JoinHandle;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use netholdem_protocol::{Request, Response};

use crate::state;

pub fn spawn(
    state_handle: state::Handle,
    stopped: state::StoppedRx,
    stream: TcpStream,
    addr: SocketAddr,
) -> (SocketAddr, JoinHandle<()>) {
    (
        addr,
        tokio::spawn(async move {
            if let Err(e) = process_connection(state_handle, stopped, stream, addr).await {
                error!("while handling {}; error = {:?}", addr, e);
            }
        }),
    )
}

async fn process_connection(
    _state: state::Handle,
    stopped_rx: watch::Receiver<bool>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let (_tx, rx) = mpsc::unbounded_channel();
    let mut client = Client::new(stream, rx, stopped_rx).await?;
    let mut hard_stop = false;

    while let Some(msg) = client.next().await {
        match msg {
            Ok(Message::Stop) => {
                info!("received notification to stop processing {}", addr);
                hard_stop = true;
                break;
            }
            Ok(Message::Request(_req)) => {
                debug!("received request from {}", addr);
            }
            Ok(Message::Response(resp)) => match serde_json::to_string(&resp) {
                Ok(json) => {
                    if let Err(e) = client.lines.send(&json).await {
                        error!("while sending response to {}: {}", addr, e);
                    }
                }
                Err(e) => error!("while serializing response to {}: {}", addr, e),
            },
            Err(e) => {
                error!("processing {}: {}", addr, e);
            }
        }
    }

    info!("disconnecting from {}", addr);

    if hard_stop {
        return Ok(())
    }

    // TODO: handle client disconnection cleanup.
    Ok(())
}

/// The state for each connected client.
struct Client {
    /// The TCP socket wrapped with the `Lines` codec, defined below.
    ///
    /// This handles sending and receiving data on the socket. When using
    /// `Lines`, we can work at the line level instead of having to manage the
    /// raw byte operations.
    lines: Framed<TcpStream, LinesCodec>,

    /// Receive half of the response channel.
    ///
    /// This is used to receive responses from server. When one is received
    /// off of this `Rx`, it will be written to the socket.
    rx: state::ResponseRx,

    /// Receive half of the stop watch.
    ///
    /// This is used to receive notification that the server has begun
    /// shutting down.
    stopped_rx: watch::Receiver<bool>,
}

impl Client {
    async fn new(
        stream: TcpStream,
        rx: state::ResponseRx,
        stopped_rx: watch::Receiver<bool>,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Client {
            lines: Framed::new(stream, LinesCodec::new()),
            rx,
            stopped_rx,
        })
    }
}

impl Stream for Client {
    type Item = Result<Message, ClientError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // If shutting down, that's the highest-priority message to handle.
        if let Poll::Ready(Some(true)) = Pin::new(&mut self.stopped_rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(Message::Stop)));
        }

        // If we've received a response, that's the next-highest priority.
        if let Poll::Ready(Some(resp)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(Message::Response(resp))));
        }

        // Last, check if there's a request line from the client.
        let line: Option<Result<String, _>> =
            futures::ready!(Pin::new(&mut self.lines).poll_next(cx));
        if let Some(Err(e)) = line {
            // Some error occurred in the line framing.
            Poll::Ready(Some(Err(ClientError::Lines { e })))
        } else if let Some(Ok(line)) = line {
            // Parse request JSON into a message.
            let request_result: Result<Request, _> = serde_json::from_str(&line);
            Poll::Ready(Some(
                request_result
                    .map(|r| Message::Request(r))
                    .map_err(|e| ClientError::Json { e }),
            ))
        } else {
            // Nothing is ready.
            Poll::Ready(None)
        }
    }
}

/// While processing a client's messages, either a TCP stream line error or a
/// JSON deserialization error may occur.
#[derive(Debug, Snafu)]
enum ClientError {
    Lines { e: LinesCodecError },
    Json { e: serde_json::Error },
}

/// A message for the client task to handle.
enum Message {
    /// The server is shutting down, halt all further processing.
    Stop,

    /// The server must send a response to the client.
    Response(Response),

    /// The server received a request from the client.
    Request(Request),
}
