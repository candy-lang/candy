use crate::{
    compiler::{hir_to_lir::HirToLir, lir::Lir},
    database::Database,
    input::{Input, InputDb},
};
use async_trait::async_trait;
use std::{future::Future, sync::Arc};
use tokio::sync::Mutex;

#[async_trait]
pub trait UseProvider {
    async fn use_asset(&self, input: Input) -> Result<Vec<u8>, String>;
    async fn use_local_module(&self, input: Input) -> Option<Lir>;
}

pub struct DbUseProvider {
    pub db: Arc<Mutex<Database>>,
}
#[async_trait]
impl UseProvider for DbUseProvider {
    async fn use_asset(&self, input: Input) -> Result<Vec<u8>, String> {
        self.db
            .lock()
            .await
            .get_input(input.clone())
            .map(|bytes| (*bytes).clone())
            .ok_or_else(|| format!("Couldn't import file '{}'.", input))
    }

    async fn use_local_module(&self, input: Input) -> Option<Lir> {
        self.db.lock().await.lir(input).map(|lir| (*lir).clone())
    }
}

pub struct FunctionUseProvider<'a, U1, F1, U2, F2>
where
    U1: (Fn(Input) -> F1) + Send + Sync,
    F1: Future<Output = Result<Vec<u8>, String>> + Send,
    U2: (Fn(Input) -> F2) + Send + Sync,
    F2: Future<Output = Option<Lir>> + Send,
{
    pub use_asset: &'a U1,
    pub use_local_module: &'a U2,
}
#[async_trait]
impl<'a, U1, F1, U2, F2> UseProvider for FunctionUseProvider<'a, U1, F1, U2, F2>
where
    U1: (Fn(Input) -> F1) + Send + Sync,
    F1: Future<Output = Result<Vec<u8>, String>> + Send,
    U2: (Fn(Input) -> F2) + Send + Sync,
    F2: Future<Output = Option<Lir>> + Send,
{
    async fn use_asset(&self, input: Input) -> Result<Vec<u8>, String> {
        (self.use_asset)(input).await
    }

    async fn use_local_module(&self, input: Input) -> Option<Lir> {
        (self.use_local_module)(input).await
    }
}
