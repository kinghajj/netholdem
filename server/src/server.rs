use std::error::Error;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_local_bounded_channel as spsc_async;
use futures::future::{select, Either};
use futures::pin_mut;
use futures::SinkExt;
use log::{debug, error, info};
use tokio;
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot, watch};

use typenum::U2;
use warp::filters::ws::{Message, WebSocket};
use warp::Filter;

use netholdem_protocol::{Request, Response};

use crate::{requests, settings, state};

/// Execute the entire life-cycle of the netholdem server.
pub async fn run(
    server: settings::Server,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<Stats, Box<dyn Error>> {
    // Create the global state.
    let guard = Arc::new(state::guard());

    // Keep track of some basic statistics.
    let total_accepted_connections = Arc::new(AtomicUsize::new(0));
    let accepted_connections = total_accepted_connections.clone();

    // Define our routes

    // Serve static client files
    let client_files = warp::get().and(warp::fs::dir(server.client_files_path));

    // Accept websocket connections
    let websocket_server = warp::path("server")
        .and(warp::ws())
        .and(warp::addr::remote())
        .map(move |ws: warp::ws::Ws, addr: Option<SocketAddr>| {
            let handle = guard.new_client();
            let total_accepted_connections = total_accepted_connections.clone();
            ws.on_upgrade(move |stream| async move {
                if let Some(addr) = addr {
                    tokio::spawn(async move {
                        total_accepted_connections.fetch_add(1, Ordering::Release);
                        info!("accepted connection from {}", addr);
                        handle_client(handle, stream, addr).await;
                    });
                } else {
                    error!("no address for incoming connection")
                }
            })
        });
    let routes = client_files.or(websocket_server);

    // Start the server
    let bind_addr = server
        .bind_addr
        .to_socket_addrs()
        .expect("valid bind address")
        .next()
        .expect("at least one address");
    let (addr, server) = warp::serve(routes).bind_with_graceful_shutdown(bind_addr, async move {
        shutdown_rx.await.ok();
        info!("received shutdown signal");
    });
    info!("running on {}", addr);
    server.await;

    info!("good-bye, world!");
    Ok(Stats {
        total_accepted_connections: accepted_connections.load(Ordering::Acquire),
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

async fn handle_client(handle: state::ClientHandle, stream: WebSocket, addr: SocketAddr) {
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
    info!("finished handling {}", addr);
}

async fn process_connection(
    mut stopped_rx: watch::Receiver<bool>,
    mut stream: WebSocket,
    addr: SocketAddr,
    mut response_rx: mpsc::Receiver<Response>,
    mut request_tx: RequestTx<'_>,
) -> Result<(), Box<dyn Error>> {
    debug!("starting connection processing loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop processing {}", addr);
                break;
            },
            Some(resp) = response_rx.next() => {
                match bincode::serialize(&resp) {
                    Ok(data) => {
                        if let Err(e) = stream.send(Message::binary(data)).await {
                            error!("while sending response to {}: {}", addr, e);
                        }
                    }
                    Err(e) => error!("while serializing response to {}: {}", addr, e),
                }
            },
            opt_msg = stream.next() => if let Some(msg) = opt_msg {
                debug!("received a request from {}", addr);
                match msg{
                    Ok(msg) => {
                        let data = msg.into_bytes();
                        match bincode::deserialize(&data) {
                            Ok(req) => {
                                if request_tx.send(req).await.is_err() {
                                    debug!("apparent death of sibling task for {}", addr);
                                    break;

                                }
                            },
                            Err(e) => error!("deserializing request from {}: {}", addr, e),
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
