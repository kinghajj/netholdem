#![warn(rust_2018_idioms)]

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
