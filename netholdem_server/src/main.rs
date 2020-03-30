#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::env;

use docopt::Docopt;
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use serde::Deserialize;

pub const USAGE: &'static str = "
netholdem_server - text-based networked Texas hold'em.

Usage:
    netholdem_server <addr> [--port=<n>]
    netholdem_server  (-h | --help)
    netholdem_server  --version

Options:
    -h --help       Show this screen.
    -V --version    Show version.
    --port=<n>      Port to bind or connect to. [default: 8023]
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_addr: String,
    pub flag_port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    //let state = Arc::new(Mutex::new(Shared::new()));

    let addr = format!("{}:{}", args.arg_addr, args.flag_port);

    // Bind a TCP listener to the socket address.
    //
    // Note that this is the Tokio TcpListener, which is fully async.
    let mut listener = TcpListener::bind(&addr).await?;

    println!("server running on {}", addr);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;

        // Clone a handle to the `Shared` state for the new connection.
        // let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            //if let Err(e) = process(state, stream, addr).await {
            //    println!("an error occurred; error = {:?}", e);
            //}
        });
    }
}
