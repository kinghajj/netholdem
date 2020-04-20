use std::error::Error;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_local_bounded_channel as spsc_async;
use futures::channel::mpsc;
use futures::future::{select, Either};
use futures::pin_mut;
use futures::SinkExt;
use log::{debug, error, info};
use tokio;
use tokio::stream::StreamExt;
use tokio::sync::{oneshot, watch};

use typenum::U2;
use warp::filters::ws::{Message, WebSocket};
use warp::Filter;

use netholdem_game::protocol::{Request, Response};
use netholdem_game::server::Core;

use crate::settings;

/// Execute the entire life-cycle of the netholdem server.
pub async fn run(
    server: settings::Server,
    game: netholdem_game::server::Settings,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<Stats, Box<dyn Error>> {
    // Create the global state.
    let guard = make_guard(game);

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

async fn handle_client(handle: ClientHandle, stream: WebSocket, addr: SocketAddr) {
    // setup communication channels
    let (state, stopped_rx) = handle.split();
    let (response_tx, response_rx) = mpsc::channel(RESPONSE_CAPACITY);
    let mut requests = spsc_async::channel::<Request, RequestCapacity>();
    let (request_tx, request_rx) = requests.split();
    // setup task loops
    let connection = process_connection(stopped_rx.clone(), stream, addr, response_rx, request_tx);
    let request = handle_requests(state, stopped_rx, addr, request_rx, response_tx);
    pin_mut!(connection, request);
    // run task loops
    let remaining = select(connection, request).await.factor_first().1;
    // wait for whoever is left to finish
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
) {
    debug!("starting connection processing loop for {}", addr);
    loop {
        tokio::select! {
            // Server shutting down
            Some(stop) = stopped_rx.next() =>
                if stop { break },
            // Write out response to socket
            Some(resp) = response_rx.next() =>
                send_response(&resp, &mut stream, &addr).await,
            // Receive request from socket
            msg = stream.next() =>
                if forward_request(msg, &mut request_tx, &addr).await {
                    break;
                }
        }
    }
}

async fn send_response(resp: &Response, stream: &mut WebSocket, addr: &SocketAddr) {
    match bincode::serialize(&resp) {
        Ok(data) => {
            if let Err(e) = stream.send(Message::binary(data)).await {
                error!("while sending response to {}: {}", addr, e);
            }
        }
        Err(e) => error!("while serializing response to {}: {}", addr, e),
    }
}

async fn forward_request(
    msg: Option<Result<Message, warp::Error>>,
    request_tx: &mut RequestTx<'_>,
    addr: &SocketAddr,
) -> bool {
    if msg.is_none() {
        return true;
    }
    let msg = msg.unwrap();
    match msg {
        Ok(msg) => {
            let data = msg.into_bytes();
            if data.len() == 0 {
                return true;
            }
            match bincode::deserialize(&data) {
                Ok(req) => {
                    if request_tx.send(req).await.is_err() {
                        return true;
                    }
                }
                Err(e) => error!("deserializing request from {}: {}", addr, e),
            }
        }
        Err(e) => error!("reading line from {}: {}", addr, e),
    }
    false
}

async fn handle_requests(
    state: Arc<State>,
    mut stopped_rx: watch::Receiver<bool>,
    addr: SocketAddr,
    mut request_rx: RequestRx<'_>,
    response_tx: mpsc::Sender<Response>,
) {
    let mut hard_stop = false;
    let mut context = state.get_core().register(response_tx).await;

    debug!("starting request handling loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop handling {}", addr);
                hard_stop = true;
                break;
            },
            opt_request = request_rx.receive() => match opt_request {
                Ok(req) => context.execute(req).await,
                Err(_) => {
                    debug!("apparent death of sibling task for {}", addr);
                    break;
                },
            }
        }
    }

    if !(hard_stop || state.stopping()) {
        debug!("cleaning up {}", addr);
        context.cleanup().await;
    }
}

/// The global state of the whole server.
pub struct State {
    stopping: AtomicBool,
    core: Arc<Core>,
}

impl State {
    /// Create a new, empty server state.
    fn new(settings: netholdem_game::server::Settings) -> Self {
        State {
            stopping: AtomicBool::new(false),
            core: Arc::new(Core::new(settings)),
        }
    }

    pub fn get_core(&self) -> &Arc<Core> {
        &self.core
    }

    /// Inquire whether the server is in the process of shutting down.
    pub fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }
}

/// Create a new, empty server state, and return a guard for it.
pub fn make_guard(settings: netholdem_game::server::Settings) -> Arc<Guard> {
    let state = Arc::new(State::new(settings));
    let (stopped_tx, stopped_rx) = watch::channel(false);
    let guard = Guard {
        state,
        stopped_tx,
        stopped_rx,
    };
    Arc::new(guard)
}

/// Ensures that client tasks receive notification of server shutdown.
///
/// This type implements `Drop`, on which it uses a combination of an atomic
/// bool and a Tokio watch to notify client connection tasks that the server
/// has begun shutdown. The main server loop should be arranged so that no
/// matter how it exits, this guard gets dropped.
pub struct Guard {
    state: Arc<State>,
    stopped_tx: watch::Sender<bool>,
    stopped_rx: watch::Receiver<bool>,
}

impl<'a> Guard {
    /// Create a handle for a new incoming client.
    pub fn new_client(&self) -> ClientHandle {
        ClientHandle {
            state: self.state.clone(),
            stopped_rx: self.stopped_rx.clone(),
        }
    }
}

impl<'a> Drop for Guard {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::Release);
        self.stopped_tx
            .broadcast(true)
            .expect("at least our own watch receiver left");
    }
}

/// A handle to the state and shutdown notifications for new clients.
#[derive(Clone)]
pub struct ClientHandle {
    state: Arc<State>,
    stopped_rx: watch::Receiver<bool>,
}

impl ClientHandle {
    /// Consume the handle to acquire its members.
    pub fn split(self) -> (Arc<State>, watch::Receiver<bool>) {
        (self.state, self.stopped_rx)
    }
}
