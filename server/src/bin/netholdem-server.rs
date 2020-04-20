#![warn(rust_2018_idioms)]

use std::error::Error;
use std::str::FromStr;

use flexi_logger::LogSpecBuilder;
use log::{error, warn, LevelFilter};
use tokio;
use tokio::sync::oneshot;

use netholdem_server::{run, settings};

fn main() -> Result<(), Box<dyn Error>> {
    let settings = settings::load()?;
    setup_logger(&settings.logging)?;
    let mut runtime = setup_runtime(&settings.runtime)?;

    runtime.block_on(async move {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            if let Err(e) = run(settings.server, settings.game, shutdown_rx).await {
                error!("server stopped: {}", e);
            }
        });
        tokio::signal::ctrl_c().await.expect("setup signal handler");
        shutdown_tx.send(()).expect("server still running");
        if let Err(e) = server.await {
            error!("server task: {}", e);
        }
    });
    Ok(())
}

fn setup_logger(l: &settings::Logging) -> Result<(), Box<dyn Error>> {
    let mut spec_builder = LogSpecBuilder::new();
    spec_builder.default(LevelFilter::from_str(&l.level)?);
    let spec = spec_builder.build();
    flexi_logger::Logger::with(spec)
        .format(|w, now, r| flexi_logger::default_format(w, now, r))
        .start()?;
    Ok(())
}

fn setup_runtime(r: &settings::Runtime) -> Result<tokio::runtime::Runtime, Box<dyn Error>> {
    let mut builder = tokio::runtime::Builder::default();
    let adjusted_max_threads = if r.core_threads >= r.max_threads {
        let max_threads = r.core_threads + 1;
        warn!(
            "max_threads must be greater than core_threads; adjusting to {}",
            max_threads
        );
        max_threads
    } else {
        r.max_threads
    };
    builder
        .enable_all()
        .core_threads(r.core_threads)
        .max_threads(adjusted_max_threads)
        .thread_name(&r.thread_name);
    if r.threaded {
        builder.threaded_scheduler();
    } else {
        builder.basic_scheduler();
    }
    Ok(builder.build()?)
}
