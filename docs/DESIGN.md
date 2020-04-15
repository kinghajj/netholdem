# Design (WIP)

## Workspace

* `client` - the client program, embedded as WebAssembly in the UI.
* `game` - core logic for Texas Hold'em.
* `model` - key data structures for the game, client, and server.
* `protocol` - request/response DTOs exchanged between the server and clients.
* `server` - the HTTP/Websocket server program.
* `ui` - the web frontend.

## Protocol

The protocol between the client and server is pretty straightforward: a long-
lived Websocket stream exchanges requests and responses to/from the server and
client, respectively. These messages are binary-encoded with `bincode`. Every
request sent by the client will result in one, and only one, response. The
server will, however, also send unsolicited responses, to update clients of
relevant changes to the game state.

## Server

The server uses tokio to be completely asynchronous. The state is kept in an a
shared mutex. The top level of the state will contain globally relevant details,
such as the set of clients, players, and open rooms. The state for each room,
though, is kept in separate shared mutexes. This way, when clients play in a
room, their actions will only need to synchronize on the room's mutex, and not
the global state mutex.

## Client

The client program core is written in Rust and compiled to WebAssembly. The UI
is written in TypeScript with React, binding user actions to calls into the
client core; the core, in turn, updates its state, and notifies React components
to be re-rendered.
