#![warn(rust_2018_idioms)]

pub use rs_poker::core::{Card, Deck, FlatDeck, Hand};
use serde::{Deserialize, Serialize};

///
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Player {
    name: String,
}

/// For simplicity, money is represented as discreet, indivisible units.
/// Two billion ought to be enough for anybody!
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Money(i32);

///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct RoomId(u32);
