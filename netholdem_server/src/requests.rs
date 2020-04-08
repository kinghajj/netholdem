use log::error;

use netholdem_protocol::{IntroductionRequest, IntroductionResponse, Request, Response};
use Request::*;

use crate::requests::Phase::*;
use crate::state;

pub enum Phase {
    NewClient,
    RegisteredPlayer,
    Playing,
}

impl Phase {
    pub async fn handle(
        self,
        state: &state::Shared,
        client: &mut state::Client,
        req: Request,
    ) -> Self {
        match self {
            NewClient => handle_new_client(state, client, req).await,
            RegisteredPlayer => handle_registered_player(state, client, req).await,
            Playing => handle_playing(state, client, req).await,
        }
    }
}

async fn handle_new_client(
    state: &state::Shared,
    client: &mut state::Client,
    req: Request,
) -> Phase {
    match req {
        Introduction(IntroductionRequest { player }) => {
            let (response, phase) =
                match state.lock().await.register_player(client.address(), player) {
                    Err(state::Error::PlayerNameTaken) => (
                        Response::Introduction(IntroductionResponse::NameAlreadyInUse),
                        NewClient,
                    ),
                    Err(state::Error::ClientAlreadyNamed) => (Response::Illegal, NewClient),
                    Ok(_) => (
                        Response::Introduction(IntroductionResponse::Success),
                        RegisteredPlayer,
                    ),
                };
            client
                .send(response)
                .await
                .map_err(|e| error!("while sending response to {}: {}", client.address(), e));
            phase
        }
        _ => {
            client
                .send(Response::Illegal)
                .await
                .map_err(|e| error!("while sending response to {}: {}", client.address(), e));
            NewClient
        }
    }
}

async fn handle_registered_player(
    _state: &state::Shared,
    _client: &mut state::Client,
    _req: Request,
) -> Phase {
    RegisteredPlayer
}

async fn handle_playing(
    _state: &state::Shared,
    _client: &mut state::Client,
    _req: Request,
) -> Phase {
    Playing
}
