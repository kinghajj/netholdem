/// The core business logic of the server.
use std::collections::BTreeMap;
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
    pub async fn register(&self, response_tx: mpsc::Sender<protocol::Response>) -> Context<'_> {
        Context::new(self, response_tx)
    }

    async fn new_room(&self, cfg: model::RoomConfig) -> Synced<Room> {
        let room_id = model::RoomId(self.next_room_id.fetch_add(1, Ordering::SeqCst));
        let room = Room::new(room_id, cfg);
        let room = make_synced(room);
        self.rooms.lock().await.insert(room_id, room.clone());
        room
    }
}

/// The handle by which client tasks may send requests to the core.
pub struct Context<'core> {
    core: &'core Core,
    response_tx: mpsc::Sender<protocol::Response>,
    phase: Phase,
}

impl<'core> Context<'core> {
    fn new(core: &'core Core, response_tx: mpsc::Sender<protocol::Response>) -> Self {
        Context {
            core,
            response_tx,
            phase: New,
        }
    }

    /// Send a request to the core.
    ///
    /// This does not return a value, but rather sends a response to the
    /// channel provided to `Core::register`.
    pub async fn execute(&mut self, req: protocol::Request) {
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
    async fn handle(&mut self, req: protocol::Request) -> protocol::Response {
        let phase = std::mem::replace(&mut self.phase, New);
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
    New,
    // If a client has introduced themselves and the client ID is remembered,
    // they are in the `Pending` phase, until they successfully authenticate.
    Pending {
        status: Synced<ClientStatus>,
        challenge: [u8; protocol::CHALLENGE_SIZE],
        secret: SecretKey,
    },
    // Once a client has introduced themselves successfully--and authenticated,
    // if they are returning--they are in the `Validated` phase. They may only
    // create or join rooms in which to play.
    Validated {
        status: Synced<ClientStatus>,
    },
    // Clients which have created or joined rooms are in the `Playing` phase.
    Playing {
        status: Synced<ClientStatus>,
        room: Synced<Room>,
    },
}

impl Phase {
    // Main request handling function.
    async fn handle<'core>(
        self,
        ctx: &mut Context<'core>,
        req: protocol::Request,
    ) -> (Self, protocol::Response) {
        use protocol::Request::*;
        match self {
            New => {
                use protocol::IntroductionResponse::*;
                use ClientRegistration::*;
                if let &Introduction(ref intro) = &req {
                    if &intro.version.0 == crate::GAME_VERSION {
                        let (client_reg, status) =
                            ctx.core.clients.lock().await.register(&intro.client_id);
                        match client_reg {
                            NotRemembered { server_id } => {
                                (Validated { status }, NiceToMeetYou { server_id }.into())
                            }
                            Remembered { challenge, secret } => (
                                Pending {
                                    status,
                                    challenge,
                                    secret,
                                },
                                YouSeemFamiliar { challenge }.into(),
                            ),
                        }
                    } else {
                        (self, MismatchedGameVersions.into())
                    }
                } else {
                    (self, protocol::Response::Illegal)
                }
            }
            Pending {
                status,
                challenge,
                secret,
            } => {
                use protocol::AuthenticationResponse::*;
                if let &Authentication(ref auth) = &req {
                    if let Ok(tag) = Tag::from_slice(&auth.tag) {
                        let res = authenticate_verify(&tag, &secret, &challenge);
                        if res.is_ok() {
                            // todo: send back relevant state information!
                            (Validated { status }, Valid.into())
                        } else {
                            (
                                Pending {
                                    status,
                                    challenge,
                                    secret,
                                },
                                Invalid.into(),
                            )
                        }
                    } else {
                        (
                            Pending {
                                status,
                                challenge,
                                secret,
                            },
                            Invalid.into(),
                        )
                    }
                } else {
                    (
                        Pending {
                            status,
                            challenge,
                            secret,
                        },
                        protocol::Response::Illegal,
                    )
                }
            }
            Validated { status } => match req {
                CreateRoom(cr_req) => {
                    use protocol::CreateRoomResponse;
                    // create the room
                    let room = ctx.core.new_room(cr_req.config).await;
                    // creator auto-joins the room.
                    let room_id = {
                        let mut room = room.lock().await;
                        room.add_member(ctx, &cr_req.player).await;
                        room.room_id
                    };
                    (
                        Playing {
                            status,
                            room: room.clone(),
                        },
                        CreateRoomResponse { room_id }.into(),
                    )
                }
                JoinRoom(jr_req) => {
                    use protocol::JoinRoomResponse::*;
                    if let Some(room) = ctx.core.rooms.lock().await.lookup(jr_req.room_id) {
                        let response = room.lock().await.join(ctx, &jr_req.player).await;
                        let phase = if response == Success.into() {
                            Playing {
                                status,
                                room: room.clone(),
                            }
                        } else {
                            Validated { status }
                        };
                        (phase, response)
                    } else {
                        (Validated { status }, NotFound.into())
                    }
                }
                _ => (Validated { status }, protocol::Response::Illegal),
            },
            Playing { .. } => (self, protocol::Response::Illegal),
        }
    }
}

struct Clients {
    clients: BTreeMap<model::EndpointId, Client>,
}

impl Clients {
    fn new() -> Self {
        Clients {
            clients: BTreeMap::new(),
        }
    }

    fn register(
        &mut self,
        client_id: &model::EndpointId,
    ) -> (ClientRegistration, Synced<ClientStatus>) {
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
                let server_id = model::EndpointId(server_public_key.as_bytes().clone());
                let ret = ClientRegistration::NotRemembered { server_id };
                // record the new client
                let client = e.insert(Client::new(shared_secret));
                (ret, client.status.clone())
            }
            // recognized client
            Occupied(mut e) => {
                // construct a random challenge
                let secret = SecretKey::from_slice(e.get().shared_secret.as_bytes())
                    .expect("key to map successfully");
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
        server_id: model::EndpointId,
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
    room_id: Option<model::RoomId>,
}

impl ClientStatus {
    fn new() -> Self {
        Self { room_id: None }
    }
}

// The set of rooms available on the server.
struct Rooms {
    rooms: BTreeMap<model::RoomId, Synced<Room>>,
}

impl Rooms {
    fn new() -> Self {
        Rooms {
            rooms: BTreeMap::new(),
        }
    }

    fn insert(&mut self, room_id: model::RoomId, room: Synced<Room>) {
        self.rooms.insert(room_id, room);
    }

    fn lookup(&self, room_id: model::RoomId) -> Option<&Synced<Room>> {
        self.rooms.get(&room_id)
    }
}

struct Room {
    room_id: model::RoomId,
    cfg: model::RoomConfig,
    members: Vec<Member>,
    // game: State,
}

impl Room {
    fn new(room_id: model::RoomId, cfg: model::RoomConfig) -> Self {
        Room {
            room_id,
            cfg,
            members: vec![],
        }
    }

    pub async fn join(
        &mut self,
        context: &mut Context<'_>,
        player: &model::Player,
    ) -> protocol::Response {
        use protocol::JoinRoomResponse::*;
        use protocol::Response::JoinRoom;
        if self.is_full() {
            JoinRoom(RoomFull)
        } else if self.contains_player(player) {
            JoinRoom(NameInUse)
        } else {
            self.add_member(context, player).await;
            JoinRoom(Success)
        }
    }

    fn is_full(&self) -> bool {
        self.members.len() == self.cfg.max_players as usize
    }

    fn contains_player(&self, p: &model::Player) -> bool {
        self.members.iter().any(|m| &m.player == p)
    }

    async fn add_member(&mut self, context: &Context<'_>, player: &model::Player) {
        use protocol::NewMemberResponse;
        use protocol::Response::NewMember;

        // add new member to collection
        self.members.push(Member {
            player: player.clone(),
            response_tx: context.response_tx.clone(),
        });
        // tell other members of the new arrival
        let event = NewMember(NewMemberResponse {
            player: player.clone(),
        });
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
    player: model::Player,
    response_tx: mpsc::Sender<protocol::Response>,
}

impl Member {
    async fn send(&mut self, r: protocol::Response) {
        self.response_tx
            .send(r)
            .await
            .map_err(|e| error!("while sending response: {}", e))
            .ok();
    }
}

type Synced<T> = Arc<Mutex<T>>;

fn make_synced<T>(t: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(t))
}
