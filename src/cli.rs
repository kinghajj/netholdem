use serde::Deserialize;

pub const USAGE: &'static str = "
netholdem - text-based networked Texas hold'em.

Usage:
    netholdem client <server> [--port=<n>]
    netholdem server <addr> [--port=<n>]
    netholdem (-h | --help)
    netholdem --version

Options:
    -h --help       Show this screen.
    -V --version    Show version.
    --port=<n>      Port to bind or connect to. [default: 8023]
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_server: String,
    pub arg_addr: String,
    pub cmd_client: bool,
    pub cmd_server: bool,
    pub flag_port: u16,
}

