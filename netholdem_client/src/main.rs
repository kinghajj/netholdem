#![warn(rust_2018_idioms)]

use std::error::Error;

use docopt::Docopt;
use serde::Deserialize;

pub const USAGE: &'static str = "
netholdem_client - text-based networked Texas hold'em.

Usage:
    netholdem_client <server> [--port=<n>]
    netholdem_client (-h | --help)
    netholdem_client --version

Options:
    -h --help       Show this screen.
    -V --version    Show version.
    --port=<n>      Port to bind or connect to. [default: 8023]
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_server: String,
    pub flag_port: u16,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    println!("Running client!");
    Ok(())
}
