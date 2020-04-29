/// The core business logic of the server.
use std::collections::BTreeMap;
use std::convert::From;
use std::default::Default;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use futures::channel::mpsc;
use futures::lock::Mutex;
use futures::SinkExt;
use log::error;
use orion::auth::{authenticate_verify, SecretKey, Tag};
use rand::rngs::OsRng;
use serde::Deserialize;

use crate::{model, protocol};
use model::*;
use protocol::{Request, Response};

use Phase::*;

pub struct Core {
    settings: Settings,
    next_room_id: AtomicU32,
    clients: Mutex<Clients>,
    rooms: Mutex<Rooms>,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct Settings {
    pub max_players_cap: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { max_players_cap: 8 }
    }
}

impl Core {
    /// Create a new, empty server core.
    pub fn new(settings: Settings) -> Self {
        Core {
            settings,
            next_room_id: AtomicU32::new(0),
            clients: Mutex::new(Clients::new()),
            rooms: Mutex::new(Rooms::new()),
        }
    }

    /// Register a new client with the core.
    ///
    /// The response channel should have a consumer that somehow delivers the
    /// responses to the client. In the actual server, this would involve
    /// serializing and writing the response to a WebSocket; in a test, the
    /// client would have the receiving channel.
    ///
    /// The returned context provides the client-handling task the means to
    /// execute incoming requests.
    pub async fn register(&self, tx: mpsc::Sender<Response>) -> Context<'_> {
        Context::new(self, tx)
    }

    async fn new_room(&self, cfg: RoomConfig) -> Synced<Room> {
        let room_id = self.next_room_id.fetch_add(1, Ordering::SeqCst);
        let room_id = RoomId(room_id);
        let room = Room::new(room_id, cfg);
        let room = make_synced(room);
        self.rooms.lock().await.insert(room_id, room.clone());
        room
    }
}

/// The handle by which client tasks may send requests to the core.
pub struct Context<'core> {
    core: &'core Core,
    response_tx: mpsc::Sender<Response>,
    phase: Phase,
}

impl<'core> Context<'core> {
    fn new(core: &'core Core, response_tx: mpsc::Sender<Response>) -> Self {
        Context {
            core,
            response_tx,
            phase: New(NewPhase),
        }
    }

    /// Send a request to the core.
    ///
    /// This does not return a value, but rather sends a response to the
    /// channel provided to `Core::register`.
    pub async fn execute(&mut self, req: Request) {
        let response = self.handle(req).await;
        self.response_tx
            .send(response)
            .await
            .map_err(|e| error!("while sending response: {}", e))
            .ok();
    }

    /// Cleanup data for this client from the core, e.g. due to disconnection.
    ///
    /// This would be better done as a Drop destructor, but, unfortunately,
    /// those don't support async yet.
    pub async fn cleanup(&mut self) {
        // TODO!
    }

    // Top-level request-handling function.
    async fn handle(&mut self, req: Request) -> Response {
        let phase = std::mem::replace(&mut self.phase, New(NewPhase));
        let (phase, response) = phase.handle(self, req).await;
        self.phase = phase;
        response
    }
}

// Represents the protocol phase of a client.
enum Phase {
    // A client that has just connected and registered, but not yet introduced
    // themselves, is in the `New` phase. The only valid request from such
    // clients is an introduction.
    New(NewPhase),
    // If a client has introduced themselves and the client ID is remembered,
    // they are in the `Pending` phase, until they successfully authenticate.
    Pending(PendingPhase),
    // Once a client has introduced themselves successfully--and authenticated,
    // if they are returning--they are in the `Validated` phase. They may only
    // create or join rooms in which to play.
    Validated(ValidatedPhase),
    // Clients which have created or joined rooms are in the `Playing` phase.
    Playing(PlayingPhase),
}

struct NewPhase;

impl From<NewPhase> for Phase {
    fn from(p: NewPhase) -> Self {
        Phase::New(p)
    }
}

struct PendingPhase {
    status: Synced<ClientStatus>,
    challenge: [u8; protocol::CHALLENGE_SIZE],
    secret: SecretKey,
}

impl From<PendingPhase> for Phase {
    fn from(p: PendingPhase) -> Self {
        Phase::Pending(p)
    }
}

struct ValidatedPhase {
    status: Synced<ClientStatus>,
}

impl From<ValidatedPhase> for Phase {
    fn from(p: ValidatedPhase) -> Self {
        Phase::Validated(p)
    }
}

struct PlayingPhase {
    status: Synced<ClientStatus>,
    room: Synced<Room>,
}

impl From<PlayingPhase> for Phase {
    fn from(p: PlayingPhase) -> Self {
        Phase::Playing(p)
    }
}

impl Phase {
    // Main request handling function.
    async fn handle<'core>(self, ctx: &mut Context<'core>, req: Request) -> (Self, Response) {
        use Request::*;
        use Response::Illegal;
        match self {
            New(p) => match req {
                Introduction(intro) => {
                    let (phase, resp) = handle_intro(intro, p, ctx).await;
                    (phase, resp.into())
                }
                _ => (New(p), Illegal),
            },
            Pending(p) => match req {
                Authentication(auth) => {
                    let (phase, resp) = handle_auth(auth, p, ctx).await;
                    (phase, resp.into())
                }
                _ => (Pending(p), Illegal),
            },
            Validated(p) => match req {
                Lobby(l) => {
                    let (phase, resp) = handle_lobby(l, p, ctx).await;
                    (phase, resp.into())
                }
                _ => (Validated(p), Illegal),
            },
            Playing { .. } => (self, Illegal),
        }
    }
}

struct Clients {
    clients: BTreeMap<EndpointId, Client>,
}

impl Clients {
    fn new() -> Self {
        Clients {
            clients: BTreeMap::new(),
        }
    }

    fn register(&mut self, client_id: &EndpointId) -> (ClientRegistration, Synced<ClientStatus>) {
        use std::collections::btree_map::Entry::{Occupied, Vacant};
        match self.clients.entry(client_id.clone()) {
            // unrecognized client
            Vacant(e) => {
                // generate server-side ephemeral secret and public key
                let secret = x25519_dalek::EphemeralSecret::new(&mut OsRng);
                let server_public_key = x25519_dalek::PublicKey::from(&secret);
                // to the DH exchange with the client public key
                let public_key = x25519_dalek::PublicKey::from(client_id.0);
                let shared_secret = secret.diffie_hellman(&public_key);
                // construct the registration response
                let server_id = server_public_key.as_bytes().clone();
                let server_id = EndpointId(server_id);
                let ret = ClientRegistration::NotRemembered { server_id };
                // record the new client
                let client = e.insert(Client::new(shared_secret));
                (ret, client.status.clone())
            }
            // recognized client
            Occupied(mut e) => {
                // construct a random challenge
                let secret = e.get().shared_secret.as_bytes();
                let secret = SecretKey::from_slice(secret);
                let secret = secret.expect("key to map successfully");
                let challenge: [u8; protocol::CHALLENGE_SIZE] = rand::random();
                let ret = ClientRegistration::Remembered { challenge, secret };
                let client = e.get_mut();
                (ret, client.status.clone())
            }
        }
    }
}

enum ClientRegistration {
    NotRemembered {
        server_id: EndpointId,
    },
    Remembered {
        challenge: [u8; protocol::CHALLENGE_SIZE],
        secret: SecretKey,
    },
}

struct Client {
    shared_secret: x25519_dalek::SharedSecret,
    status: Synced<ClientStatus>,
}

impl Client {
    fn new(shared_secret: x25519_dalek::SharedSecret) -> Self {
        Self {
            shared_secret,
            status: make_synced(ClientStatus::new()),
        }
    }
}

struct ClientStatus {
    room_id: Option<RoomId>,
}

impl ClientStatus {
    fn new() -> Self {
        Self { room_id: None }
    }
}

// The set of rooms available on the server.
struct Rooms {
    rooms: BTreeMap<RoomId, Synced<Room>>,
}

impl Rooms {
    fn new() -> Self {
        Rooms {
            rooms: BTreeMap::new(),
        }
    }

    fn insert(&mut self, room_id: RoomId, room: Synced<Room>) {
        self.rooms.insert(room_id, room);
    }

    fn lookup(&self, room_id: RoomId) -> Option<&Synced<Room>> {
        self.rooms.get(&room_id)
    }
}

struct Room {
    room_id: RoomId,
    cfg: RoomConfig,
    members: Vec<Member>,
    // game: State,
}

impl Room {
    fn new(room_id: RoomId, cfg: RoomConfig) -> Self {
        Room {
            room_id,
            cfg,
            members: vec![],
        }
    }

    pub async fn join(
        &mut self,
        context: &mut Context<'_>,
        player: &Player,
    ) -> protocol::JoinRoomResponse {
        use protocol::JoinRoomResponse::*;
        if self.is_full() {
            RoomFull
        } else if self.contains_player(player) {
            NameInUse
        } else {
            self.add_member(context, player).await;
            JoinedRoom
        }
    }

    fn is_full(&self) -> bool {
        self.members.len() == self.cfg.max_players as usize
    }

    fn contains_player(&self, p: &Player) -> bool {
        self.members.iter().any(|m| &m.player == p)
    }

    async fn add_member(&mut self, context: &Context<'_>, player: &Player) {
        use protocol::PlayResponse;

        // add new member to collection
        self.members.push(Member {
            player: player.clone(),
            response_tx: context.response_tx.clone(),
        });
        // tell other members of the new arrival
        let event: Response = PlayResponse::NewMember {
            player: player.clone(),
        }
        .into();
        for m in self.members.iter_mut() {
            // but don't tell the new player, since they get a response anyway.
            if &m.player == player {
                continue;
            }
            m.send(event.clone()).await;
        }
    }
}

struct Member {
    player: Player,
    response_tx: mpsc::Sender<Response>,
}

impl Member {
    async fn send(&mut self, r: Response) {
        self.response_tx
            .send(r)
            .await
            .map_err(|e| error!("while sending response: {}", e))
            .ok();
    }
}

// request handling

async fn handle_intro<'core>(
    intro: protocol::IntroductionRequest,
    _: NewPhase,
    ctx: &mut Context<'core>,
) -> (Phase, protocol::IntroductionResponse) {
    use protocol::IntroductionResponse::*;
    use ClientRegistration::*;

    if &intro.version.0 != crate::GAME_VERSION {
        return (NewPhase.into(), MismatchedGameVersions);
    }

    let (client_reg, status) = {
        let mut clients = ctx.core.clients.lock().await;
        clients.register(&intro.client_id)
    };
    match client_reg {
        NotRemembered { server_id } => (
            ValidatedPhase { status }.into(),
            NiceToMeetYou { server_id },
        ),
        Remembered { challenge, secret } => (
            PendingPhase {
                status,
                challenge,
                secret,
            }
            .into(),
            YouSeemFamiliar { challenge },
        ),
    }
}

async fn handle_auth<'core>(
    auth: protocol::AuthenticationRequest,
    phase: PendingPhase,
    _: &mut Context<'core>,
) -> (Phase, protocol::AuthenticationResponse) {
    let PendingPhase {
        status,
        challenge,
        secret,
    } = phase;
    use protocol::AuthenticationResponse::*;
    if let Ok(tag) = Tag::from_slice(&auth.tag) {
        let res = authenticate_verify(&tag, &secret, &challenge);
        if res.is_ok() {
            // todo: send back relevant state information!
            (ValidatedPhase { status }.into(), Valid)
        } else {
            (
                PendingPhase {
                    status,
                    challenge,
                    secret,
                }
                .into(),
                Invalid,
            )
        }
    } else {
        (
            PendingPhase {
                status,
                challenge,
                secret,
            }
            .into(),
            Invalid,
        )
    }
}

async fn handle_lobby<'core>(
    l: protocol::LobbyRequest,
    phase: ValidatedPhase,
    ctx: &mut Context<'core>,
) -> (Phase, protocol::LobbyResponse) {
    use protocol::LobbyRequest::*;
    match l {
        CreateRoom(cr) => {
            let (phase, resp) = handle_create_room(cr, phase, ctx).await;
            (phase, resp.into())
        }
        JoinRoom(jr) => {
            let (phase, resp) = handle_join_room(jr, phase, ctx).await;
            (phase, resp.into())
        }
    }
}

async fn handle_create_room<'core>(
    cr: protocol::CreateRoomRequest,
    phase: ValidatedPhase,
    ctx: &mut Context<'core>,
) -> (Phase, protocol::CreateRoomResponse) {
    use protocol::CreateRoomResponse;
    let ValidatedPhase { status } = phase;

    // create the room
    let room = ctx.core.new_room(cr.config).await;
    // creator auto-joins the room.
    let room_id = {
        let mut room = room.lock().await;
        room.add_member(ctx, &cr.player).await;
        room.room_id
    };
    (
        PlayingPhase {
            status,
            room: room.clone(),
        }
        .into(),
        CreateRoomResponse { room_id },
    )
}

async fn handle_join_room<'core>(
    jr: protocol::JoinRoomRequest,
    phase: ValidatedPhase,
    ctx: &mut Context<'core>,
) -> (Phase, protocol::JoinRoomResponse) {
    use protocol::JoinRoomResponse::*;
    let ValidatedPhase { status } = phase;
    if let Some(room) = ctx.core.rooms.lock().await.lookup(jr.room_id) {
        let response = room.lock().await.join(ctx, &jr.player).await;
        let phase = if response == JoinedRoom {
            PlayingPhase {
                status,
                room: room.clone(),
            }
            .into()
        } else {
            ValidatedPhase { status }.into()
        };
        (phase, response)
    } else {
        (ValidatedPhase { status }.into(), RoomNotFound)
    }
}

type Synced<T> = Arc<Mutex<T>>;

fn make_synced<T>(t: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(t))
}
