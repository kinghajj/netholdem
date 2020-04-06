use std::collections::{btree_map, hash_map, BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use snafu::Snafu;
use tokio::sync::{mpsc, watch, Mutex, MutexGuard};

use netholdem_model::{Player, RoomId};
use netholdem_protocol::Response;

/// The global state of the whole server.
pub struct State {
    stopping: AtomicBool,
    synced: Mutex<Synced>,
}

impl State {
    /// Create a new, empty server state.
    fn new() -> Self {
        State {
            stopping: AtomicBool::new(false),
            synced: Mutex::new(Synced::new()),
        }
    }

    /// Wait for access to the synced state.
    pub async fn lock(&self) -> MutexGuard<'_, Synced> {
        self.synced.lock().await
    }

    /// Inquire whether the server is in the process of shutting down.
    pub fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }
}

/// A shared handle to the global server state.
pub type Shared = Arc<State>;

/// Create a new, empty server state, and return a guard for it.
pub fn guard() -> Guard {
    let state = Arc::new(State::new());
    let (stopped_tx, stopped_rx) = watch::channel(false);
    Guard {
        state,
        stopped_tx,
        stopped_rx,
    }
}

/// Ensures that clients receive notification of server shutdown.
///
/// This type implements `Drop`, on which it uses a combination of an atomic
/// bool and a Tokio watch to notify client connection tasks that the server
/// has begun shutdown. The main server loop should be arranged so that no
/// matter how it exits, this guard gets dropped.
pub struct Guard {
    state: Shared,
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
pub struct ClientHandle {
    state: Shared,
    stopped_rx: watch::Receiver<bool>,
}

impl ClientHandle {
    /// Consume the handle to acquire its members.
    pub fn split(self) -> (Shared, watch::Receiver<bool>) {
        (self.state, self.stopped_rx)
    }
}

/// The mutex-synchronized state.
pub struct Synced {
    /// A collection of each connected client.
    clients: HashMap<SocketAddr, Client>,
    /// A mapping of each client to their player name.
    players: HashMap<SocketAddr, Player>,
    /// An inverse mapping of each player name to the respective client.
    r_players: BTreeMap<Player, SocketAddr>,
    /// A collection of each open room.
    rooms: HashMap<RoomId, Arc<Mutex<Room>>>,
}

impl Synced {
    fn new() -> Self {
        Synced {
            clients: HashMap::new(),
            players: HashMap::new(),
            r_players: BTreeMap::new(),
            rooms: HashMap::new(),
        }
    }

    pub fn register_new_client(&mut self, addr: SocketAddr, tx: ResponseTx) -> Client {
        let client = Client::new(addr, tx);
        self.clients.insert(addr, client.clone());
        client
    }

    pub fn contains_player(&self, p: &Player) -> bool {
        self.r_players.contains_key(p)
    }

    pub fn register_player(&mut self, addr: SocketAddr, p: Player) -> Result<(), Error> {
        match self.players.entry(addr) {
            hash_map::Entry::Occupied(_) => Err(Error::ClientAlreadyNamed),
            hash_map::Entry::Vacant(player) => match self.r_players.entry(p.clone()) {
                btree_map::Entry::Occupied(_) => Err(Error::PlayerNameTaken),
                btree_map::Entry::Vacant(r_player) => {
                    player.insert(p);
                    r_player.insert(addr);
                    Ok(())
                }
            },
        }
    }

    pub fn already_named(&self, addr: SocketAddr) -> bool {
        self.players.contains_key(&addr)
    }

    pub fn cleanup_client(&mut self, addr: SocketAddr) {
        // TODO: remove client from room they may be in.
        if let Some(p) = self.players.remove(&addr) {
            self.r_players.remove(&p);
        }
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    PlayerNameTaken,
    ClientAlreadyNamed,
}

/// A handle for each client connected to the server.
#[derive(Clone)]
pub struct Client {
    addr: SocketAddr,
    tx: ResponseTx,
}

impl Client {
    pub fn new(addr: SocketAddr, tx: ResponseTx) -> Self {
        Client { addr, tx }
    }

    pub fn address(&self) -> SocketAddr {
        self.addr
    }

    pub fn send(&mut self, r: Response) -> Result<(), mpsc::error::SendError<Response>> {
        self.tx.send(r)
    }
}

pub struct Room {
    members: HashMap<Player, Client>,
    // game: netholdem_game::State,
}

/// The sender half for responses to a client.
pub type ResponseTx = mpsc::UnboundedSender<Response>;

/// The receiver half for responses to a client.
pub type ResponseRx = mpsc::UnboundedReceiver<Response>;
