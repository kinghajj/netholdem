#![warn(rust_2018_idioms)]

use serde::{Deserialize, Serialize};

use netholdem_model::{Player, RoomId};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Request {
    Introduction(IntroductionRequest),
    JoinRoom(JoinRoomRequest),
    SitIn(SitInRequest),
    SitOut(SitOutRequest),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Response {
    Illegal,
    Introduction(IntroductionResponse),
    JoinRoom(JoinRoomResponse),
    SitIn(SitInResponse),
    SitOut(SitOutResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntroductionRequest {
    pub player: Player,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum IntroductionResponse {
    Success,
    NameAlreadyInUse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JoinRoomRequest {
    pub room_id: RoomId,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum JoinRoomResponse {
    Success,
    RoomFull,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SitInRequest;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SitInResponse {
    Success,
    AlreadySatIn,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SitOutRequest;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SitOutResponse {
    Success,
    AlreadySatOut,
}
