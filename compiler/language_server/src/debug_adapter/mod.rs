use self::session::run_debug_session;
use crate::server::Server;
use dap::{prelude::EventBody, requests::Request, responses::Response};
use derive_more::{Display, From};
use lsp_types::notification::Notification;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::thread;
use tokio::sync::{mpsc, RwLock};
use tower_lsp::{jsonrpc, Client};
use tracing::error;

mod session;

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SessionId(String);

/// Manages the debug sessions.
///
/// # Communication
///
/// Requests, responses, and events defined in the Debug Adapter Protocol (DAP)
/// are sent as notifications via the Language Server Protocol (LSP).
///
/// Messages from the client (IDE) to the server ([`DebugSession`]) land in
/// [`Server::candy_debug_adapter_message`], which calls
/// [`handle_message`]. Based on the [`SessionId`] included in the message, the
/// message is forwarded to the corresponding [`DebugSession`]. Each session
/// runs in its own thread and has a channel for receiving messages.
///
/// Messages from the server to the client are sent directly.
#[derive(Debug, Default)]
pub struct DebugSessionManager {
    sessions: RwLock<FxHashMap<SessionId, mpsc::Sender<Request>>>,
}
impl DebugSessionManager {
    async fn create_session(&mut self, session_id: SessionId, client: Client) {
        let (client_to_server_sender, client_to_server_receiver) = mpsc::channel(4);

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), client_to_server_sender);
        }

        thread::spawn(|| run_debug_session(session_id, client, client_to_server_receiver));
    }
    async fn handle_message(&self, request: RequestNotification) {
        let sessions = self.sessions.read().await;
        let Some(session) = sessions.get(&request.session_id) else {
            error!(
                "No debug session found with ID {}.",
                request.session_id,
            );
            return;
        };
        session.send(request.message).await.unwrap();
    }
}

impl Server {
    pub async fn candy_debug_adapter_create(
        &self,
        params: DebugSessionCreateParams,
    ) -> jsonrpc::Result<()> {
        let mut state = self.require_running_state_mut().await;
        state
            .debug_session_manager
            .create_session(params.session_id, self.client.clone())
            .await;
        Ok(())
    }

    pub async fn candy_debug_adapter_message(&self, params: RequestNotification) {
        let state = self.require_running_state().await;
        state.debug_session_manager.handle_message(params).await;
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSessionCreateParams {
    pub session_id: SessionId,
}

// Client to Server
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestNotification {
    pub session_id: SessionId,
    pub message: Request,
}

// Server to Client
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerToClient {
    pub session_id: SessionId,
    pub message: ServerToClientMessage,
}
impl Notification for ServerToClient {
    const METHOD: &'static str = "candy/debugAdapter/message";

    type Params = Self;
}

// [`dap::responses::Response`] is missing `"type": "response"` in its JSON
// representation. Therefore, we add the `"type"` field here and use the raw
// [`EventBody`] for events.
#[derive(Debug, From, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerToClientMessage {
    Response(Response),
    Event(EventBody),
}
// Even though we only ever send this notification, `tower_lsp` still requires it to be deserializeable.
impl<'de> Deserialize<'de> for ServerToClientMessage {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        panic!("ServerToClientMessage is not deserializable.")
    }
}
