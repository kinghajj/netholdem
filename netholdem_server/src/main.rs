#![warn(rust_2018_idioms)]

use std::error::Error;

use tokio;

mod cli;
mod client;
mod requests;
mod server;
mod state;

// TODO: maybe create runtime manually, for finer control?
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    flexi_logger::Logger::with_env()
        .format(|w, now, r| flexi_logger::colored_with_thread(w, now, r))
        .start()?;
    server::run(cli::parse_args()?).await
}
