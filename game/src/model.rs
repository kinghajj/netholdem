pub use rs_poker::core::{Card, Deck, FlatDeck, Hand};
use serde::{Deserialize, Serialize};

///
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Player {
    pub name: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ClientId(pub u64);

/// For simplicity, money is represented as discreet, indivisible units.
/// Two billion ought to be enough for anybody!
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Money(i32);

///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct RoomId(pub u32);

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct RoomConfig {
    pub name: String,
    pub max_players: u8,
    pub keyphrase: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct GameVersion(pub String);
