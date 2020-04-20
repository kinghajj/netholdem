use serde::{Deserialize, Serialize};
use std::default::Default;

use crate::model;
use crate::model::{Player, RoomConfig, RoomId};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Request {
    Introduction(IntroductionRequest),
    CreateRoom(CreateRoomRequest),
    JoinRoom(JoinRoomRequest),
    SitIn(SitInRequest),
    SitOut(SitOutRequest),
}

impl Request {
    pub fn success(&self) -> Response {
        use Request::*;
        match self {
            Introduction(_) => Response::Introduction(IntroductionResponse::Success),
            JoinRoom(_) => Response::JoinRoom(JoinRoomResponse::Success),
            SitIn(_) => Response::SitIn(SitInResponse::Success),
            SitOut(_) => Response::SitOut(SitOutResponse::Success),
            _ => panic!("this request has no default success response"),
        }
    }

    pub fn typical_error(&self) -> Response {
        use Request::*;
        match self {
            Introduction(_) => Response::Introduction(IntroductionResponse::MismatchedGameVersions),
            JoinRoom(_) => Response::JoinRoom(JoinRoomResponse::RoomFull),
            SitIn(_) => Response::SitIn(SitInResponse::AlreadySatIn),
            SitOut(_) => Response::SitOut(SitOutResponse::AlreadySatOut),
            _ => panic!("this request has no default error response"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Response {
    Illegal,
    Introduction(IntroductionResponse),
    CreateRoom(CreateRoomResponse),
    JoinRoom(JoinRoomResponse),
    NewMember(NewMemberResponse),
    SitIn(SitInResponse),
    SitOut(SitOutResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntroductionRequest {
    pub version: model::GameVersion,
}

impl Default for IntroductionRequest {
    fn default() -> Self {
        IntroductionRequest {
            version: model::GameVersion(crate::GAME_VERSION.into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum IntroductionResponse {
    Success,
    MismatchedGameVersions,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CreateRoomRequest {
    pub config: RoomConfig,
    pub player: Player,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CreateRoomResponse {
    pub room_id: RoomId,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JoinRoomRequest {
    pub room_id: RoomId,
    pub player: Player,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum JoinRoomResponse {
    Success,
    NotFound,
    RoomFull,
    NameInUse,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NewMemberResponse {
    pub player: Player,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SitInRequest;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SitInResponse {
    Success,
    AlreadySatIn,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SitOutRequest;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SitOutResponse {
    Success,
    AlreadySatOut,
}
