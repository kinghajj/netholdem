use docopt::Docopt;
use serde::Deserialize;

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

pub fn parse_args() -> Args {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit())
}
