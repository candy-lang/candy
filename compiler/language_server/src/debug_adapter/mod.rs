use self::adapter::DebugAdapter;
use crate::server::Server;
use dap::requests::Request;
use derive_more::Display;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc;
use tracing::{debug, error};

mod adapter;

// import_types!("debugAdapterProtocol.json");

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
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
        let adapter = DebugAdapter {
            session_id: params.session_id.clone(),
            client: self.client.clone(),
        };
        let mut state = self.require_running_state_mut().await;
        state
            .debug_adapter_server
            .adapters
            .insert(params.session_id, RwLock::new(adapter));
        Ok(())
    }

    pub async fn candy_debug_adapter_message(&self, params: serde_json::Value) {
        let params: RequestNotification = serde_json::from_value(params).unwrap();
        debug!("Received debug adapter message: {:?}", params.message);
        let state = self.require_running_state().await;
        let Some(adapter) = state
            .debug_adapter_server
            .adapters
            .get(&params.session_id) else {
            error!(
                "No debug adapter found with id {}.",
                params.session_id,
            );
            return;
        };
        let mut adapter = adapter.write().await;
        adapter.handle(params.message).await;
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugAdapterCreateParams {
    pub session_id: SessionId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestNotification {
    pub session_id: SessionId,
    pub message: Request,
}
