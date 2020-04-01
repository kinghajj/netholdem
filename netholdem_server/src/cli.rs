use docopt::Docopt;
use serde::Deserialize;
use std::error::Error;

pub const USAGE: &'static str = "
netholdem_server - text-based networked Texas hold'em.

Usage:
    netholdem_server <bind-addr>
    netholdem_server  (-h | --help)
    netholdem_server  --version

Options:
    -h --help       Show this screen.
    -V --version    Show version.
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_bind_addr: String,
}

pub fn parse_args() -> Result<Args, Box<dyn Error>> {
    Ok(Docopt::new(USAGE).and_then(|d| d.deserialize())?)
}
