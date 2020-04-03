use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;

use snafu::Snafu;
use tokio::sync::{mpsc, watch, Mutex};

use netholdem_model::{Player, RoomId};
use netholdem_protocol::Response;
use std::collections::{btree_map, hash_map};

/// The global state of the whole server.
pub struct State {
    /// A collection of each connected client.
    clients: HashMap<SocketAddr, Client>,
    /// A mapping of each client to their player name.
    players: HashMap<SocketAddr, Player>,
    /// An inverse mapping of each player name to the respective client.
    r_players: BTreeMap<Player, SocketAddr>,
    /// A collection of each open room.
    rooms: HashMap<RoomId, Arc<Mutex<Room>>>,
}

impl State {
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

pub type Handle = Arc<Mutex<State>>;

pub fn create() -> Handle {
    Arc::new(Mutex::new(State::new()))
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

impl State {
    fn new() -> Self {
        State {
            clients: HashMap::new(),
            players: HashMap::new(),
            r_players: BTreeMap::new(),
            rooms: HashMap::new(),
        }
    }
}

/// The sender half for responses to a client.
pub type ResponseTx = mpsc::UnboundedSender<Response>;

/// The receiver half for responses to a client.
pub type ResponseRx = mpsc::UnboundedReceiver<Response>;

pub type StoppedTx = watch::Sender<bool>;
pub type StoppedRx = watch::Receiver<bool>;
