use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::env;

use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use crate::cli::Args;

struct State {

}

pub async fn run(args: Args) -> Result<(), Box<dyn Error>> {
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

