# Design (WIP)

## Workspace

* `game` - core logic for the game and client/server interaction.
* `server` - the HTTP/Websocket server program.
* `ui` - the web client frontend.

## Game

### Protocol

The protocol between the client and server is pretty straightforward: a long-
lived Websocket stream exchanges requests and responses to/from the server and
client, respectively. These messages are binary-encoded with `bincode`. Every
request sent by the client will result in one, and only one, response. The
server will, however, also send unsolicited responses, to update clients of
relevant changes to the game state.

To support clients on unreliable connections, each has a client ID. An ID is a
public key created using `x25519_dalek` crate. Clients introduce themselves to
the server with this ID, and the server remembers it and associates its internal
state for handling that client with the ID. This way, if a client disconnects,
and later reconnects and introduces itself with the same ID, the server can
issue a challenge to authenticate the client. If the client succeeds, the server
will respond with the last-remembered state of the client, so that it may resume
from where it left off.

### Client & Server Cores

The core logic for both the client and server lives here, and is written so as
to be decoupled from the actual transport mechanism. The server core receives
concurrent requests, executes them, and sends out responses; the client core,
likewise, generates requests, receives responses, and reacts accordingly. This
allows the cores to be tested together as a unit.

## Server

The server uses tokio to be completely asynchronous. The state is kept in an a
shared mutex. The top level of the state will contain globally relevant details,
such as the set of clients and open rooms. The state for each room, though, is
kept in separate shared mutexes. This way, when clients play in a room, their
actions will only need to synchronize on the room's mutex, and not the global
state mutex.

## UI

The client program core is written in Rust and compiled to WebAssembly. The UI
is written using Yew, binding user actions to calls into the client core; the
core, in turn, updates its state, and notifies components to be re-rendered.
Components then use the core state to render themselves appropriately.
