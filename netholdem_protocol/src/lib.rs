#![warn(rust_2018_idioms)]

use serde::{Deserialize, Serialize};

use netholdem_model::{Player, RoomId};

#[derive(Clone, Deserialize, Serialize)]
pub enum Request {
    Introduction(IntroductionRequest),
    JoinRoom(JoinRoomRequest),
    SitIn(SitInRequest),
    SitOut(SitOutRequest),
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Response {
    Introduction(IntroductionResponse),
    JoinRoom(JoinRoomResponse),
    SitIn(SitInResponse),
    SitOut(SitOutResponse),
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IntroductionRequest {
    player: Player,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum IntroductionResponse {
    Success,
    NameAlreadyInUse,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct JoinRoomRequest {
    room_id: RoomId,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum JoinRoomResponse {
    Success,
    RoomFull,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SitInRequest;

#[derive(Clone, Deserialize, Serialize)]
pub enum SitInResponse {
    Success,
    AlreadySatIn,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SitOutRequest;

#[derive(Clone, Deserialize, Serialize)]
pub enum SitOutResponse {
    Success,
    AlreadySatOut,
}
