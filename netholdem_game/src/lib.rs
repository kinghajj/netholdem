#![warn(rust_2018_idioms)]
// TODO: remove this!
#![allow(dead_code)]

use std::collections::HashMap;

use netholdem_model::{FlatDeck, Hand, Player};

struct State {
    deck: FlatDeck,
    players: HashMap<Player, Option<Hand>>,
}

struct PlayerState {
    hand: Option<Hand>,
}

enum Phase {}
