use std::error::Error;
use std::net::SocketAddr;


use futures::{SinkExt};
use log::{debug, error};

use tokio;

use tokio::net::TcpStream;
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio::task::JoinHandle;
use tokio_util::codec::{Framed, LinesCodec};

use netholdem_protocol::{IntroductionResponse, Request, Response};

use crate::{requests, state};

pub fn spawn(
    state_handle: state::Handle,
    stopped: state::StoppedRx,
    stream: TcpStream,
    addr: SocketAddr,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = process_connection(state_handle, stopped, stream, addr).await {
            error!("while handling {}; error = {:?}", addr, e);
        }
    })
}

async fn process_connection(
    state: state::Handle,
    mut stopped_rx: watch::Receiver<bool>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let (responses_tx, mut responses_rx) = mpsc::unbounded_channel();
    let mut lines = Framed::new(stream, LinesCodec::new());
    let mut hard_stop = false;

    let mut client = state.lock().await.register_new_client(addr, responses_tx);
    let mut phase = requests::Phase::NewClient;
    //let mut handler = requests::initial_handler(&state, client);

    loop {
        tokio::select! {
            Some(true) = stopped_rx.next() => {
                debug!("received notification to stop processing {}", addr);
                hard_stop = true;
                break;
            },
            Some(resp) = responses_rx.next() =>
                match serde_json::to_string(&resp) {
                    Ok(json) => {
                        if let Err(e) = lines.send(&json).await {
                            error!("while sending response to {}: {}", addr, e);
                        }
                    }
                    Err(e) => error!("while serializing response to {}: {}", addr, e),
                },
            Some(line) = lines.next() =>
                match line {
                    Ok(line) => {
                        match serde_json::from_str::<Request>(&line) {
                            Ok(req) => phase = phase.handle(&state, &mut client, req).await,
                            Err(e) => error!("parsing request from {}: {}", addr, e),
                        }
                    },
                    Err(e) => error!("reading line from {}: {}", addr, e),
                },
        }
    }

    debug!("disconnecting from {}", addr);

    if !hard_stop {
        state.lock().await.cleanup_client(addr);
    }

    // TODO: handle client disconnection cleanup.
    Ok(())
}
