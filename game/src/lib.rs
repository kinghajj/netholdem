#![warn(rust_2018_idioms)]
// TODO: remove this!
#![allow(dead_code)]

use std::collections::HashMap;

use crate::model::{FlatDeck, Hand, Player};

pub mod client;
pub mod model;
pub mod protocol;
pub mod server;

struct State {
    deck: FlatDeck,
    players: HashMap<Player, Option<Hand>>,
}

struct PlayerState {
    hand: Option<Hand>,
}

enum Phase {}

const GAME_VERSION: &'static str = env!("CARGO_PKG_VERSION");
