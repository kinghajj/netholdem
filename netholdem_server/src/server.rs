use std::error::Error;
use std::net::SocketAddr;

use async_local_bounded_channel as spsc_async;
use futures::future::{select, Either, FutureExt};
use futures::pin_mut;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::SinkExt;
use log::{debug, error, info};
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_util::codec::{Framed, LinesCodec};
use typenum::U2;

use netholdem_protocol::{Request, Response};

use crate::{requests, settings, state};

/// Execute the entire life-cycle of the netholdem server.
pub async fn run(
    server: settings::Server,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<Stats, Box<dyn Error>> {
    let mut listener = TcpListener::bind(&server.bind_addr).await?;
    let mut client_tasks = FuturesUnordered::new();
    let mut total_accepted_connections = 0;
    {
        let guard = state::guard();
        // Main loop: wait for something interesting, like...
        info!("running on {}", server.bind_addr);
        loop {
            tokio::select! {
                // shutdown signal
                Ok(()) = (&mut shutdown_rx) => {
                    info!("received shutdown signal");
                    // Inform connection tasks of shutdown.
                    drop(guard);
                    // Stop accepting further connections.
                    break
                },
                // inbound connection
                Ok((stream, addr)) = listener.accept() => {
                    debug!("accepted connection from {}", addr);
                    total_accepted_connections += 1;
                    let task = spawn_client(&guard, stream, addr);
                    client_tasks.push(task);
                },
                // completed connection task
                Some(result) = client_tasks.next() => {
                    if let Err(e) = result {
                        error!("{}", e);
                    }
                },
            }
        }
    }

    // Gracefully shutdown and await all connection tasks.
    info!("reaping {} client tasks", client_tasks.len());
    while let Some(result) = client_tasks.next().await {
        if let Err(e) = result {
            error!("{}", e);
        }
    }

    info!("good-bye, world!");
    Ok(Stats {
        total_accepted_connections,
    })
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Stats {
    pub total_accepted_connections: usize,
}

// The limit of pending responses waiting to be sent to a particular client.
// This should be a bit higher than the maximum number of responses we expect
// to send to a client as a result of a request by *any other client*.
const RESPONSE_CAPACITY: usize = 4;

// The limit of pending requests from a particular client.
type RequestCapacity = U2;

type RequestTx<'a> = spsc_async::Sender<'a, Request, RequestCapacity>;
type RequestRx<'a> = spsc_async::Receiver<'a, Request, RequestCapacity>;

type ClientTask = JoinHandle<()>;

fn spawn_client(guard: &state::Guard, stream: TcpStream, addr: SocketAddr) -> ClientTask {
    let handle = guard.new_client();
    tokio::spawn(async move {
        let (state, stopped_rx) = handle.split();
        let (response_tx, response_rx) = mpsc::channel(RESPONSE_CAPACITY);
        let mut requests = spsc_async::channel::<Request, RequestCapacity>();
        let (request_tx, request_rx) = requests.split();
        let connection = {
            let stopped_rx = stopped_rx.clone();
            async move {
                let result =
                    process_connection(stopped_rx, stream, addr, response_rx, request_tx).await;
                if let Err(e) = result {
                    error!("while processing {}; error = {:?}", addr, e);
                }
            }
        };
        let request = async move {
            let result = handle_requests(state, stopped_rx, addr, request_rx, response_tx).await;
            if let Err(e) = result {
                error!("while handling {}; error = {:?}", addr, e);
            }
        };

        pin_mut!(connection, request);
        let remaining = select(connection, request).await.factor_first().1;
        match remaining {
            Either::Left(f) => f.await,
            Either::Right(f) => f.await,
        }
    })
}

async fn process_connection(
    mut stopped_rx: watch::Receiver<bool>,
    stream: TcpStream,
    addr: SocketAddr,
    mut response_rx: mpsc::Receiver<Response>,
    mut request_tx: RequestTx<'_>,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    debug!("starting connection processing loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop processing {}", addr);
                break;
            },
            Some(resp) = response_rx.next() => {
                match serde_json::to_string(&resp) {
                    Ok(json) => {
                        if let Err(e) = lines.send(&json).await {
                            error!("while sending response to {}: {}", addr, e);
                        }
                    }
                    Err(e) => error!("while serializing response to {}: {}", addr, e),
                }
            },
            opt_line = lines.next() => if let Some(line) = opt_line {
                debug!("received a request line from {}", addr);
                match line {
                    Ok(line) => {
                        match serde_json::from_str::<Request>(&line) {
                            Ok(req) => {
                                if let Err(_) = request_tx.send(req).await {
                                    debug!("apparent death of sibling task for {}", addr);
                                    break;
                                }
                            },
                            Err(e) => error!("parsing request from {}: {}", addr, e),
                        }
                    },
                    Err(e) => error!("reading line from {}: {}", addr, e),
                }
            } else {
                debug!("disconnection from {}", addr);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_requests(
    state: state::Shared,
    mut stopped_rx: watch::Receiver<bool>,
    addr: SocketAddr,
    mut request_rx: RequestRx<'_>,
    response_tx: mpsc::Sender<Response>,
) -> Result<(), Box<dyn Error>> {
    let mut hard_stop = false;
    let mut client = state.lock().await.register_new_client(addr, response_tx);
    let mut phase = requests::Phase::NewClient;

    debug!("starting request handling loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop handling {}", addr);
                hard_stop = true;
                break;
            },
            opt_request = request_rx.receive() => match opt_request {
                Ok(req) => phase = phase.handle(&state, &mut client, req).await,
                Err(_) => {
                    debug!("apparent death of sibling task for {}", addr);
                    break;
                },
            }
        }
    }

    if !hard_stop && !state.stopping() {
        debug!("cleaning up {}", addr);
        state.lock().await.cleanup_client(addr);
    }

    Ok(())
}
