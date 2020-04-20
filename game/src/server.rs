/// The core business logic of the server.
use std::default::Default;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use futures::channel::mpsc;
use futures::lock::Mutex;
use futures::SinkExt;
use log::error;
use serde::Deserialize;

use crate::{model, protocol};
use Phase::*;

pub struct Core {
    settings: Settings,
    next_client_id: AtomicU64,
    next_room_id: AtomicU32,
    rooms: Mutex<Rooms>,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct Settings {
    pub max_players_cap: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_players_cap: 8,
        }
    }
}

impl Core {
    /// Create a new, empty server core.
    pub fn new(settings: Settings) -> Self {
        Core {
            settings,
            next_client_id: AtomicU64::new(0),
            next_room_id: AtomicU32::new(0),
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
    pub async fn register(
        &self,
        response_tx: mpsc::Sender<protocol::Response>,
    ) -> Context<'_> {
        let client_id = model::ClientId(self.next_client_id.fetch_add(1, Ordering::SeqCst));
        Context::new(self, client_id, response_tx)
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
pub struct Context<'a> {
    core: &'a Core,
    client_id: model::ClientId,
    response_tx: mpsc::Sender<protocol::Response>,
    phase: Phase,
}

impl<'a> Context<'a> {
    fn new(
        core: &'a Core,
        client_id: model::ClientId,
        response_tx: mpsc::Sender<protocol::Response>,
    ) -> Self {
        Context {
            core,
            client_id,
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
        use protocol::Request::*;
        match self.phase {
            New => {
                if let &Introduction(ref intro) = &req {
                    if &intro.version.0 == crate::GAME_VERSION {
                        self.phase.change(Introduced);
                        return req.success();
                    } else {
                        return req.typical_error();
                    }
                }
            }
            Introduced => match req {
                CreateRoom(cr_req) => {
                    use protocol::CreateRoomResponse;
                    use protocol::Response::CreateRoom;
                    // create the room
                    let room = self.core.new_room(cr_req.config).await;
                    // creator auto-joins the room.
                    let room_id = {
                        let mut room = room.lock().await;
                        room.add_member(self, &cr_req.player).await;
                        room.room_id
                    };
                    self.phase.change(Playing { room: room.clone() });
                    return CreateRoom(CreateRoomResponse { room_id });
                }
                JoinRoom(jr_req) => {
                    use protocol::JoinRoomResponse::*;
                    use protocol::Response::JoinRoom;
                    if let Some(room) = self.core.rooms.lock().await.lookup(jr_req.room_id) {
                        return room.lock().await.join(self, &jr_req.player).await;
                    } else {
                        return JoinRoom(NotFound);
                    }
                }
                _ => {}
            },
            Playing { .. } => {}
        }

        return protocol::Response::Illegal;
    }
}

// Represents the protocol phase of a client.
enum Phase {
    // A client that has just connected and registered, but not yet introduced
    // themselves, is in the `New` phase. The only valid request from such
    // clients is an introduction.
    New,
    // Once a client has introduced themselves successfully, they are in the
    // `Introduced` phase. They may only create or join rooms in which to play.
    Introduced,
    // Clients which have created or joined rooms are in the `Playing` phase.
    Playing { room: Synced<Room> },
}

impl Phase {
    fn change(&mut self, prime: Phase) {
        std::mem::replace(self, prime);
    }
}

// The set of rooms availabe on the server.
struct Rooms {
    rooms: HashMap<model::RoomId, Synced<Room>>,
}

impl Rooms {
    fn new() -> Self {
        Rooms {
            rooms: HashMap::new(),
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
            client_id: context.client_id,
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
    client_id: model::ClientId,
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
