use std::default::Default;
use std::env;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

pub fn load() -> Result<Settings, ConfigError> {
    let mut s = Config::new();
    s.merge(File::with_name(DEFAULT_CFG_PATH))?;
    let env = env::var(RUN_MODE_ENV).unwrap_or_else(|_| "development".into());
    s.merge(File::with_name(&format!("config/{}", env)).required(false))?;
    s.merge(File::with_name(LOCAL_CFG_PATH).required(false))?;
    s.merge(Environment::with_prefix(ENV_PREFIX))?;
    s.try_into()
}

const DEFAULT_CFG_PATH: &str = "config/default";
const LOCAL_CFG_PATH: &str = "config/local";
const RUN_MODE_ENV: &str = "NETHOLDEM_SERVER_RUN_MODE";
const ENV_PREFIX: &str = "netholdem_server";

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    pub logging: Logging,
    pub runtime: Runtime,
    pub server: Server,
}

#[derive(Debug, Deserialize)]
pub struct Logging {
    pub level: String,
}

impl Default for Logging {
    fn default() -> Self {
        Logging {
            level: "info".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Runtime {
    pub threaded: bool,
    pub core_threads: usize,
    pub max_threads: usize,
    pub thread_name: String,
}

impl Default for Runtime {
    fn default() -> Self {
        let num_cores = num_cpus::get_physical();
        Runtime {
            threaded: true,
            core_threads: num_cores,
            max_threads: num_cores * 2,
            thread_name: "async-worker".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub bind_addr: String,
    pub client_files_path: String,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            bind_addr: "127.0.0.1:3000".into(),
            client_files_path: "./ui/".into(),
        }
    }
}
