#![warn(rust_2018_idioms)]

use std::error::Error;

use docopt::Docopt;
use tokio;

mod cli;
mod client;
mod protocol;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: cli::Args = Docopt::new(cli::USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_client {
        client::run(args).await
    } else if args.cmd_server {
        server::run(args).await
    } else {
        unreachable!()
    }
}
