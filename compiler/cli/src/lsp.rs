use crate::{utils::packages_path, ProgramResult};
use candy_language_server::server::Server;
use tracing::info;

pub async fn lsp() -> ProgramResult {
    info!("Starting language server…");
    let (service, socket) = Server::create(packages_path());
    tower_lsp::Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    Ok(())
}
