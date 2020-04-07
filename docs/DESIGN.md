# Design (WIP)

## Workspace

* `client` - the client program.
* `game` - core logic for Texas Hold'em.
* `model` - key data structures for the game, client, and server.
* `protocol` - request/response DTOs exchanged between the server and clients.
* `server` - the server program.

## Protocol

The protocol between the client and server is pretty straightforward: a long-
lived TCP stream exchanges requests and responses to/from the server and client,
respectively. These messages are encoded as JSON text, and each message is
delimited by a newline. Every request sent by the client will result in one,
and only one, response. The server will, however, also send unsolicited
responses, to update clients of relevant changes to the game state.

## Server

The server uses tokio to be completely asynchronous, spawning one task for each
client connection. The state is kept in an `Arc<Mutex<_>>`, a clone of which is
made for each client task. The top level of the state will contain globally
relevant details, such as the set of players and open rooms. The state for each
room, though, will also be kept in an `Arc<Mutex<_>>`. This way, when a client
is playing in a room, their actions will only need to synchronize on the room's
mutex, and not the global state mutex.

## Client

The client program is written synchronously, though with multiple threads,
using channels for concurrency.