use futures::stream::futures_unordered::FuturesUnordered;
use futures::SinkExt;
use std::time::Duration;

use tokio::stream::StreamExt;
use tokio::sync::oneshot;

use netholdem_game::protocol::{IntroductionRequest, IntroductionResponse, Request, Response};
use netholdem_game::server;
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
    let bind_addr = "127.0.0.1:8080";
    let client_bind_addr = "ws://127.0.0.1:8080/server";
    let settings = settings::Server {
        bind_addr: bind_addr.into(),
        client_files_path: "./".into(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let game = server::Settings::default();
    let server = tokio::spawn(async move { run(settings, game, shutdown_rx).await.ok() });

    // Hack: wait a bit for the server to be ready.
    tokio::time::delay_for(Duration::from_millis(150)).await;

    // Spawn many clients in parallel.
    const NUM_CLIENTS: usize = 1000;
    let mut connections = FuturesUnordered::new();
    for _id in 0..NUM_CLIENTS {
        connections.push(tokio::spawn(async move {
            match tokio_tungstenite::connect_async(client_bind_addr).await {
                Ok((mut stream, _)) => {
                    // introduce ourself
                    let intro = Request::Introduction(IntroductionRequest::default());
                    let intro_bytes = bincode::serialize(&intro).expect("serialization to work");
                    stream
                        .send(tungstenite::Message::binary(intro_bytes))
                        .await
                        .expect("server to be up");
                    // get response from server
                    let resp_bytes = stream
                        .next()
                        .await
                        .expect("server to respond")
                        .expect("response to be successful")
                        .into_data();
                    let response: Response =
                        bincode::deserialize(&resp_bytes).expect("serialization to work");
                    Ok((stream, response))
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
        assert!(match response {
            &Response::Introduction(IntroductionResponse::Success) => true,
            _ => false,
        });
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
