use std::error::Error;

use futures::stream::futures_unordered::FuturesUnordered;
use log::{debug, error, info};
use tokio;
use tokio::net::TcpListener;
use tokio::stream::StreamExt;

use crate::{cli, client, state};

/// Execute the entire life-cycle of the netholdem server.
pub async fn run(args: cli::Args) -> Result<(), Box<dyn Error>> {
    let mut listener = TcpListener::bind(&args.arg_bind_addr).await?;
    let mut connection_tasks = FuturesUnordered::new();
    {
        let guard = state::guard();
        // Main loop: wait for something interesting, like...
        info!("netholdem server running on {}", args.arg_bind_addr);
        loop {
            tokio::select! {
                // shutdown signal/Ctrl-C.
                _ = tokio::signal::ctrl_c() => {
                    info!("received shutdown signal");
                    // Inform connection tasks of shutdown.
                    drop(guard);
                    // Stop accepting further connections.
                    break
                },
                // inbound connection,
                Ok((stream, addr)) = listener.accept() => {
                    debug!("accepted connection from {}", addr);
                    // Spawn task for this connection.
                    connection_tasks.push(client::spawn(&guard, stream, addr));
                },
                // completed connection task.
                Some(result) = connection_tasks.next() => {
                    if let Err(e) = result {
                        error!("{}", e);
                    }
                },
            }
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
