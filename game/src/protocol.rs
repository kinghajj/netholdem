use std::convert::From;

use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::{Player, RoomConfig, RoomId};

/// Every possible kind of request that a client may send.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Request {
    Introduction(IntroductionRequest),
    Authentication(AuthenticationRequest),
    CreateRoom(CreateRoomRequest),
    JoinRoom(JoinRoomRequest),
    SitIn,
    SitOut,
}

/// Every possible kind of response that a server may send.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Response {
    Illegal,
    Introduction(IntroductionResponse),
    Authentication(AuthenticationResponse),
    CreateRoom(CreateRoomResponse),
    JoinRoom(JoinRoomResponse),
    NewMember(NewMemberResponse),
    SitIn(SitInResponse),
    SitOut(SitOutResponse),
}

// Auxillary macros for converting inner request/response types into their
// outermost counterparts.
macro_rules! request {
    ($ty:ident, $r:ident) => {
        impl From<$r> for Request {
            fn from(r: $r) -> Self {
                Request::$ty(r)
            }
        }
    };
}

macro_rules! response {
    ($ty:ident, $r:ident) => {
        impl From<$r> for Response {
            fn from(r: $r) -> Self {
                Response::$ty(r)
            }
        }
    };
}

/// Formal introduction of a client to the server.
///
/// After a connection has been successfully established, the client will send
/// this request. This serves two purposes: first, verify that the client uses
/// a compatible version of the game code; and second, establish a unique ID
/// for the client, which can be used in subsequent connections to authenticate
/// the client.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntroductionRequest {
    /// The version of the game code running on the client.
    ///
    /// If this does not match the server's version exactly, it replies with
    /// `MismatchedGameVersions`.
    pub version: model::GameVersion,

    /// A unique ID for the client.
    ///
    /// If the server does not recognize the ID, it replies with
    /// `NiceToMeetYou`. The client is then fully verified, since this was its
    /// first introduction.
    ///
    /// If the server _does_ recognize the ID, it replies with
    /// `YouSeemFamiliar`. The client must then make an `AuthenticationRequest`,
    /// to verify that it possesses the shared secret derived during the first
    /// introduction.
    pub client_id: model::EndpointId,
}
request!(Introduction, IntroductionRequest);

impl IntroductionRequest {
    pub fn new(client_id: model::EndpointId) -> Self {
        Self {
            version: model::GameVersion(crate::GAME_VERSION.into()),
            client_id,
        }
    }
}

/// Completion of a formal introduction of a client to the server.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum IntroductionResponse {
    /// The server does not recognized the client endpoint ID from the request,
    /// so treats the client as a new, never-before-met one. Thus, it assumes
    /// that the endpoint ID is valid, and replies with its endpoint ID, so that
    /// the client can derive the same shared secret.
    NiceToMeetYou { server_id: model::EndpointId },

    /// The server recognizes the client endpoint ID, so treats the client as
    /// a previously-met one. Thus, it challenges the client to verify it
    /// possesses the same shared secret derived during the first introduction.
    YouSeemFamiliar { challenge: [u8; CHALLENGE_SIZE] },

    /// The introduction request specified an incompatible game version.
    MismatchedGameVersions,
}

/// The length of the challenge data issued from the server.
pub const CHALLENGE_SIZE: usize = 32;

response!(Introduction, IntroductionResponse);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthenticationRequest {
    pub tag: Vec<u8>,
}

request!(Authentication, AuthenticationRequest);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum AuthenticationResponse {
    Valid,
    Invalid,
}

response!(Authentication, AuthenticationResponse);

/// Create a new room and have the client automatically join it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CreateRoomRequest {
    /// The configuration settings for the new room.
    pub config: RoomConfig,
    /// The player that this client plays as within the new room.
    pub player: Player,
}

request!(CreateRoom, CreateRoomRequest);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CreateRoomResponse {
    pub room_id: RoomId,
}

response!(CreateRoom, CreateRoomResponse);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JoinRoomRequest {
    pub room_id: RoomId,
    pub player: Player,
}

request!(JoinRoom, JoinRoomRequest);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum JoinRoomResponse {
    Success,
    NotFound,
    RoomFull,
    NameInUse,
}

response!(JoinRoom, JoinRoomResponse);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NewMemberResponse {
    pub player: Player,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SitInResponse {
    Success,
    AlreadySatIn,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SitOutResponse {
    Success,
    AlreadySatOut,
}
