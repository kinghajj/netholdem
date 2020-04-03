use std::error::Error;

use futures::stream::futures_unordered::FuturesUnordered;
use log::{debug, error, info};
use tokio;
use tokio::net::TcpListener;
use tokio::stream::StreamExt;
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
    let mut connection_tasks = FuturesUnordered::new();
    loop {
        tokio::select! {
            // shutdown signal/Ctrl-C.
            _ = tokio::signal::ctrl_c() => {
                info!("received shutdown signal");
                // Inform connection tasks of shutdown.
                stopped_tx.broadcast(true)?;
                // Stop accepting further connections.
                break
            },
            // inbound connection,
            Ok((stream, addr)) = listener.accept() => {
                debug!("accepted connection from {}", addr);
                // Spawn task for this connection.
                connection_tasks.push(
                    client::spawn(
                        state_handle.clone(),
                        stopped_rx.clone(),
                        stream,
                        addr
                ));
            },
            // completed connection tasks
            Some(result) = connection_tasks.next() => {
                if let Err(e) = result {
                    error!("{}", e);
                }
            },
        }
    }

    // Gracefully shutdown and await all connection tasks.
    info!("reaping {} connection tasks", connection_tasks.len());
    while let Some(result) = connection_tasks.next().await {
        if let Err(e) = result {
            error!("{}", e);
        }
    }

    info!("good-bye, world!");
    Ok(())
}
