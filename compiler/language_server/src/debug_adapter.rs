use crate::server::Server;
use dap::{
    requests::{Command, InitializeArguments, Request},
    responses::{Response, ResponseBody},
    types::Capabilities,
};
use derive_more::Display;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc;
use tracing::debug;

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, Serialize)]
pub struct SessionId(String);

#[derive(Debug, Default)]
pub struct DebugAdapterServer {
    adapters: FxHashMap<SessionId, RwLock<DebugAdapter>>,
}

impl Server {
    pub async fn candy_debug_adapter_create(
        &self,
        params: DebugAdapterCreateParams,
    ) -> jsonrpc::Result<()> {
        let mut state = self.require_running_state_mut().await;
        state
            .debug_adapter_server
            .adapters
            .insert(params.session_id, RwLock::new(DebugAdapter {}));
        Ok(())
    }
    pub async fn candy_debug_adapter_message(
        &self,
        params: Message<Request>,
    ) -> jsonrpc::Result<Message<Response>> {
        let state = self.require_running_state().await;
        let mut adapter = state
            .debug_adapter_server
            .adapters
            .get(&params.debug_adapter_id)
            .ok_or_else(|| {
                jsonrpc::Error::invalid_params(format!(
                    "No debug adapter found with id {}.",
                    params.debug_adapter_id,
                ))
            })?
            .write()
            .await;
        let response_body = adapter.handle(params.payload.command);
        Ok(Message {
            debug_adapter_id: params.debug_adapter_id,
            payload: Response {
                request_seq: params.payload.seq,
                success: true,
                message: None,
                body: Some(response_body),
            },
        })
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugAdapterCreateParams {
    pub session_id: SessionId,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message<P> {
    pub debug_adapter_id: SessionId,
    pub payload: P,
}

#[derive(Debug)]
pub struct DebugAdapter {}

impl DebugAdapter {
    fn handle(&mut self, command: Command) -> ResponseBody {
        debug!("Received DAP request: {command:?}");
        ResponseBody::Empty
    }
}
