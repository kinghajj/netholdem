#![warn(rust_2018_idioms)]



use log::error;
use tokio;
use tokio::sync::oneshot;

mod cli;
mod client;
mod requests;
mod server;
mod state;

// TODO: maybe create runtime manually, for finer control?
#[tokio::main]
async fn main() {
    let args = cli::parse_args();
    flexi_logger::Logger::with_env()
        .format(|w, now, r| flexi_logger::colored_with_thread(w, now, r))
        .start()
        .expect("logger to start");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        if let Err(e) = server::run(args, shutdown_rx).await {
            error!("server stopped: {}", e);
        }
    });
    tokio::signal::ctrl_c().await.expect("setup signal handler");
    shutdown_tx.send(()).expect("server still running");
    if let Err(e) = server.await {
        error!("server task: {}", e);
    }
}
