#![warn(rust_2018_idioms)]

use std::str::FromStr;

use flexi_logger::LogSpecBuilder;
use futures::future::{select, Either};
use futures::pin_mut;
use futures::prelude::*;
use log::{error, info, warn, LevelFilter};
use tokio;

use netholdem_server::{run, settings};

fn main() -> anyhow::Result<()> {
    let settings = settings::load()?;
    setup_logger(&settings.logging)?;
    let signal_rx = setup_signal()?;
    let mut runtime = setup_runtime(&settings.runtime)?;

    runtime.block_on(async move {
        // Spin up the server.
        let (shutdown_tx, shutdown_rx) = piper::chan(0);
        let server = tokio::spawn(async move {
            if let Err(e) = run(settings.server, settings.game, shutdown_rx).await {
                error!("server stopped: {}", e);
            }
        });
        // Forward receiving signal to shutdown notification.
        let signal = async move {
            signal_rx.recv().await;
            info!("sending shutdown notice");
            drop(shutdown_tx);
        };
        pin_mut!(signal);
        // Wait either for receiving the signal, or for the server task to
        // stop unexpectedly (e.g. due to a panic).
        let completed = select(signal, server).await;
        match completed {
            // We've received the signal, and need to wait for the server to
            // shutdown gracefully.
            Either::Left((_, server)) => {
                if let Err(e) = server.await {
                    error!("server task: {}", e);
                }
            }
            // The server stopped but signal still hasn't been received, so
            // there's an error.
            Either::Right((res, _)) => {
                error!("server stopped unexpectedly");
                if let Err(e) = res {
                    error!("server task: {}", e);
                }
            }
        };
    });
    info!("good-bye, world!");
    Ok(())
}

fn setup_logger(l: &settings::Logging) -> anyhow::Result<()> {
    let mut spec_builder = LogSpecBuilder::new();
    spec_builder.default(LevelFilter::from_str(&l.level)?);
    let spec = spec_builder.build();
    flexi_logger::Logger::with(spec)
        .format(|w, now, r| flexi_logger::default_format(w, now, r))
        .start()?;
    Ok(())
}

fn setup_signal() -> anyhow::Result<piper::Receiver<()>> {
    let (signal_tx, signal_rx) = piper::chan(2);
    ctrlc::set_handler(move || {
        info!("received interrupt signal");
        signal_tx.send(()).now_or_never();
    })?;
    Ok(signal_rx)
}

fn setup_runtime(r: &settings::Runtime) -> anyhow::Result<tokio::runtime::Runtime> {
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
