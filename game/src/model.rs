pub use rs_poker::core::{Card, Deck, FlatDeck, Hand};
use serde::{Deserialize, Serialize};

/// A player is a client who has joined a room.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Player {
    /// The player's unique name within the room they occupy.
    pub name: String,
}

/// For simplicity, money is represented as discreet, indivisible units.
/// Two billion ought to be enough for anybody!
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Money(i32);

/// A unique identifier for a room, which hosts a game of hold'em.
///
/// Again, two billion ought to be enough for anybody!
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct RoomId(pub u32);

/// Configuration to set up a room.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct RoomConfig {
    /// The title of the room.
    pub name: String,
    /// The maximum number of players allowed in the room.
    pub max_players: u8,
    /// An optional keyphrase to authorize access to a room.
    pub keyphrase: Option<String>,
}

/// The version of this crate. Used to ensure that a client and server are
/// "speaking the same language," since messages are binary-encoded and no
/// attempts at forwards- or backwards- compatibility are made.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct GameVersion(pub String);

/// The public state of the room.
///
/// This should only convey information that *any* player should have access to.
/// Thus, it doesn't contain the deck, nor player hands.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct PublicRoomState {
    /// The members of the room.
    pub members: Vec<PublicMemberState>,
    /// The pot of money for the current deal.
    pub pot: Money,
    /// The community cards which have been dealt.
    pub community: Vec<Card>,
}

/// The public state of a room member.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct PublicMemberState {
    /// The player themself.
    pub player: Player,
    /// How much money that player has.
    pub pile: Money,
    /// The player's status within the room.
    pub status: MemberStatus,
}

/// When a client first joins a room, they are spectating. Once they request to
/// sit in, they wait until the current deal, if any, completes. Then they
/// become seated and ready for the next deal.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub enum MemberStatus {
    Spectating,
    Waiting,
    Seated,
}

/// A cryptographically-unique identifier for a particular client.
///
/// Currently, this is `x25519_dalek::PublicKey.as_bytes()`.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct EndpointId(pub [u8; ENDPOINT_ID_SIZE]);

/// The length (in bytes) of an X25519 public key.
pub const ENDPOINT_ID_SIZE: usize = 32;
