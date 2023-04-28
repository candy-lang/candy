use crate::server::Server;
use candy_frontend::id::{CountableId, IdGenerator};
use dap::{
    requests::{Command, Request},
    responses::{Response, ResponseBody},
};
use derive_more::Display;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc;
use tracing::debug;

#[derive(
    Clone, Copy, Debug, Deserialize, Display, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct DebugAdapterId(usize);
impl CountableId for DebugAdapterId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct DebugAdapterServer {
    id_generator: IdGenerator<DebugAdapterId>,
    adapters: FxHashMap<DebugAdapterId, RwLock<DebugAdapter>>,
}

impl Server {
    pub async fn candy_debug_adapter_create(
        &self,
        _params: serde_json::Value,
    ) -> jsonrpc::Result<DebugAdapterId> {
        let mut state = self.require_running_state_mut().await;
        let id = state.debug_adapter_server.id_generator.generate();
        state
            .debug_adapter_server
            .adapters
            .insert(id, RwLock::new(DebugAdapter {}));
        Ok(id)
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message<P> {
    pub debug_adapter_id: DebugAdapterId,
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
