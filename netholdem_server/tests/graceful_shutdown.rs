use futures::stream::futures_unordered::FuturesUnordered;
use futures::SinkExt;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::stream::StreamExt;
use tokio::sync::oneshot;
use tokio_util::codec::{Framed, LinesCodec};

use netholdem_model::Player;
use netholdem_protocol::{IntroductionRequest, IntroductionResponse, Request, Response};
use netholdem_server::{run, settings};

// Ensure that:
//
// - a server can be started.
// - a large number of clients can connect and submit successful requests.
// - the server receives the shutdown notification.
// - all client tasks stop.
// - the server shuts down gracefully.
#[tokio::test(core_threads = 8)]
async fn graceful_shutdown() {
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
                    let intro_line = serde_json::to_string(&intro).expect("serialization to work");
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
