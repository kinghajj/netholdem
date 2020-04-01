use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};

use netholdem_model::{Player, RoomId};
use netholdem_protocol::Response;

pub struct State {
    peers: HashMap<SocketAddr, ClientState>,
    rooms: HashMap<RoomId, Room>,
    players: HashSet<Player>,
}

pub type Handle = Arc<Mutex<State>>;

pub fn create() -> Handle {
    Arc::new(Mutex::new(State::new()))
}

pub struct ClientState {
    tx: ResponseTx,
    player: Option<Player>,
}

pub struct Room {}

impl State {
    fn new() -> Self {
        State {
            peers: HashMap::new(),
            rooms: HashMap::new(),
            players: HashSet::new(),
        }
    }
}

impl ClientState {
    fn new(tx: ResponseTx) -> Self {
        ClientState { tx, player: None }
    }
}

/// The sender half for responses to a client.
pub type ResponseTx = mpsc::UnboundedSender<Response>;

/// The receiver half for responses to a client.
pub type ResponseRx = mpsc::UnboundedReceiver<Response>;

pub type StoppedTx = watch::Sender<bool>;
pub type StoppedRx = watch::Receiver<bool>;
