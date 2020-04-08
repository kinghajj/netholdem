use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;

use futures::stream::futures_unordered::FuturesUnordered;
use futures::SinkExt;
use log::{debug, error, info};
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_util::codec::{Framed, LinesCodec};

use netholdem_protocol::{Request, Response};

use crate::{requests, settings, state};

/// Execute the entire life-cycle of the netholdem server.
pub async fn run(
    server: settings::Server,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<Stats, Box<dyn Error>> {
    let mut listener = TcpListener::bind(&server.bind_addr).await?;
    let mut client_tasks = FuturesUnordered::new();
    let mut total_accepted_connections = 0;
    {
        let guard = state::guard();
        // Main loop: wait for something interesting, like...
        info!("running on {}", server.bind_addr);
        loop {
            tokio::select! {
                // shutdown signal
                Ok(()) = (&mut shutdown_rx) => {
                    info!("received shutdown signal");
                    // Inform connection tasks of shutdown.
                    drop(guard);
                    // Stop accepting further connections.
                    break
                },
                // inbound connection
                Ok((stream, addr)) = listener.accept() => {
                    debug!("accepted connection from {}", addr);
                    total_accepted_connections += 1;
                    // Spawn task for this connection.
                    let (connection, request) = spawn_client(&guard, stream, addr);
                    client_tasks.push(connection);
                    client_tasks.push(request);
                },
                // completed connection task
                Some(result) = client_tasks.next() => {
                    if let Err(e) = result {
                        error!("{}", e);
                    }
                },
            }
        }
    }

    // Gracefully shutdown and await all connection tasks.
    info!("reaping {} client tasks", client_tasks.len());
    while let Some(result) = client_tasks.next().await {
        if let Err(e) = result {
            error!("{}", e);
        }
    }

    info!("good-bye, world!");
    Ok(Stats {
        total_accepted_connections,
    })
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Stats {
    pub total_accepted_connections: usize,
}

fn spawn_client(
    guard: &state::Guard,
    stream: TcpStream,
    addr: SocketAddr,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let handle = guard.new_client();
    let (state, stopped_rx) = handle.split();
    let stopped_rx2 = stopped_rx.clone();
    let (response_tx, response_rx) = mpsc::channel(16);
    let (request_tx, request_rx) = mpsc::channel(16);
    let connection_task = tokio::spawn(async move {
        let result = process_connection(stopped_rx, stream, addr, response_rx, request_tx).await;
        if let Err(e) = result {
            error!("while processing {}; error = {:?}", addr, e);
        }
    });
    let request_task = tokio::spawn(async move {
        let result = handle_requests(state, stopped_rx2, addr, request_rx, response_tx).await;
        if let Err(e) = result {
            error!("while handling {}; error = {:?}", addr, e);
        }
    });
    (connection_task, request_task)
}

async fn process_connection(
    mut stopped_rx: watch::Receiver<bool>,
    stream: TcpStream,
    addr: SocketAddr,
    mut response_rx: mpsc::Receiver<Response>,
    mut request_tx: mpsc::Sender<Request>,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    debug!("starting connection processing loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop processing {}", addr);
                break;
            },
            Some(resp) = response_rx.next() => {
                match serde_json::to_string(&resp) {
                    Ok(json) => {
                        if let Err(e) = lines.send(&json).await {
                            error!("while sending response to {}: {}", addr, e);
                        }
                    }
                    Err(e) => error!("while serializing response to {}: {}", addr, e),
                }
            },
            opt_line = lines.next() => if let Some(line) = opt_line {
                debug!("received a request line from {}", addr);
                match line {
                    Ok(line) => {
                        match serde_json::from_str::<Request>(&line) {
                            Ok(req) => {
                                if let Err(_) = request_tx.send(req).await {
                                    debug!("apparent death of sibling task for {}", addr);
                                    break;
                                }
                            },
                            Err(e) => error!("parsing request from {}: {}", addr, e),
                        }
                    },
                    Err(e) => error!("reading line from {}: {}", addr, e),
                }
            } else {
                debug!("disconnection from {}", addr);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_requests(
    state: state::Shared,
    mut stopped_rx: watch::Receiver<bool>,
    addr: SocketAddr,
    mut request_rx: mpsc::Receiver<Request>,
    response_tx: mpsc::Sender<Response>,
) -> Result<(), Box<dyn Error>> {
    let mut hard_stop = false;
    let mut client = state.lock().await.register_new_client(addr, response_tx);
    let mut phase = requests::Phase::NewClient;

    debug!("starting request handling loop for {}", addr);
    loop {
        tokio::select! {
            Some(stop) = stopped_rx.next() => if stop {
                debug!("received notification to stop processing {}", addr);
                hard_stop = true;
                break;
            },
            opt_request = request_rx.next() => match opt_request {
                Some(req) => phase = phase.handle(&state, &mut client, req).await,
                None => {
                    debug!("apparent death of sibling task for {}", addr);
                    break;
                },
            }
        }
    }

    if !hard_stop && !state.stopping() {
        debug!("cleaning up {}", addr);
        state.lock().await.cleanup_client(addr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::settings;
    use futures::stream::futures_unordered::FuturesUnordered;
    use futures::SinkExt;
    use std::time::Duration;

    use tokio::net::TcpStream;
    use tokio::stream::StreamExt;
    use tokio::sync::oneshot;
    use tokio_util::codec::{Framed, LinesCodec};

    use netholdem_model::Player;
    use netholdem_protocol::{IntroductionRequest, IntroductionResponse, Request, Response};

    // Ensure that:
    //
    // - a server can be started.
    // - a large number of clients can connect and submit successful requests.
    // - the server receives the shutdown notification.
    // - all client tasks stop.
    // - the server shuts down gracefully.
    #[tokio::test(core_threads = 8)]
    async fn smoke() {
        flexi_logger::Logger::with_env()
            .format(|w, now, r| flexi_logger::with_thread(w, now, r))
            .start()
            .expect("logger to start");
        // Spawn server.
        let bind_addr = "127.0.0.1:8023";
        let settings = settings::Server {
            bind_addr: bind_addr.into(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(async move { run(settings, shutdown_rx).await.ok() });

        // Hack: wait a bit for the server to be ready.
        tokio::time::delay_for(Duration::from_millis(15)).await;

        // Spawn many clients in parallel.
        const NUM_CLIENTS: usize = 1000;
        let mut connections = FuturesUnordered::new();
        for id in 0..NUM_CLIENTS {
            connections.push(tokio::spawn(async move {
                match TcpStream::connect(bind_addr).await {
                    Ok(stream) => {
                        let mut lines = Framed::new(stream, LinesCodec::new());
                        // introduce ourself
                        let intro = Request::Introduction(IntroductionRequest {
                            player: Player {
                                name: format!("player{}", id),
                            },
                        });
                        let intro_line =
                            serde_json::to_string(&intro).expect("serialization to work");
                        lines.send(intro_line).await.expect("server to be up");
                        // get response from server
                        let line = lines.next().await.expect("server to respond").expect("");
                        let response: Response =
                            serde_json::from_str(&line).expect("serialization to work");
                        Ok((lines, response))
                    }
                    Err(e) => Err(e),
                }
            }));
        }

        // Wait for all clients to get a request through.
        let mut clients = Vec::with_capacity(NUM_CLIENTS);
        while let Some(client_task) = connections.next().await {
            let client = client_task.expect("client");
            clients.push(client);
        }

        // Ensure every client successfully introduced themselves.
        for client in clients.iter() {
            let &(_, ref response) = client.as_ref().expect("clients to succeed");
            assert_eq!(
                response,
                &Response::Introduction(IntroductionResponse::Success)
            );
        }

        // Tell server to shutdown.
        shutdown_tx.send(()).expect("server still running");
        let stats = server
            .await
            .expect("server shutdown smoothly")
            .expect("server shutdown smoothly");

        // Ensure the server agrees with us.
        assert_eq!(stats.total_accepted_connections, NUM_CLIENTS);
        drop(clients);
    }
}
