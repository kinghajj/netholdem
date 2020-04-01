use std::error::Error;

use log::{error, info};
use tokio;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch, Mutex};

use crate::{cli, client, state};

/// Execute the entire lifecycle of the netholdem server.
pub async fn run(args: cli::Args) -> Result<(), Box<dyn Error>> {
    // Create initial state.
    let state_handle = state::create();
    let (stopped_tx, stopped_rx) = watch::channel(false);

    // Bind server to the address and begin listening.
    let mut listener = TcpListener::bind(&args.arg_bind_addr).await?;
    info!("netholdem server running on {}", args.arg_bind_addr);

    // Main loop: wait for incoming connections, spawn tasks for each one.
    let mut connection_tasks = Vec::new();
    loop {
        tokio::select! {
            // Asynchronously wait for an inbound TcpStream.
            Ok((stream, addr)) = listener.accept() => {
                info!("accepted connection from {}", addr);
                // Spawn task for this connection.
                connection_tasks.push(client::spawn(state_handle.clone(), stopped_rx.clone(), stream, addr));
            },
            // Asynchronously wait for shutdown signal/Ctrl-C.
            _ = tokio::signal::ctrl_c() => {
                info!("received shutdown signal");
                // Inform connection tasks of shutdown.
                stopped_tx.broadcast(true)?;
                // Stop accepting further connections.
                break
            }
        }
    }

    // Gracefully shutdown and await all connection tasks.
    info!("reaping connection tasks");
    for (addr, task) in connection_tasks.into_iter() {
        if let Err(e) = task.await {
            error!("while draining {}; error = {:?}", addr, e);
        }
    }

    info!("good-bye, world!");
    Ok(())
}
