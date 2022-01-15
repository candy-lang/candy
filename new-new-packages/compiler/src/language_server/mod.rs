// use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, InitializeResult,
    InitializedParams, MessageType, ServerCapabilities,
};
use lspower::{jsonrpc, Client, LanguageServer};
use std::error::Error;

#[derive(Debug)]
pub struct CandyLanguageServer {
    pub client: Client,
}

#[lspower::async_trait]
impl LanguageServer for CandyLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        log::info!("Initialize.");
        self.client
            .log_message(MessageType::INFO, "Initializing!")
            .await;
        Ok(InitializeResult::default())
    }

    async fn initialized(&self, _: InitializedParams) {
        log::info!("Now Initialized.");
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }
}

// pub fn run() -> Result<(), Box<dyn Error + Sync + Send>> {
//     log::info!("starting generic LSP server");

//     // Create the transport. Includes the stdio (stdin and stdout) versions but this could
//     // also be implemented to use sockets or HTTP.
//     let (connection, io_threads) = Connection::stdio();
//     log::info!("1");

//     // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
//     let server_capabilities = serde_json::to_value(&ServerCapabilities::default()).unwrap();
//     log::info!("2");
//     let initialization_params = connection.initialize(server_capabilities)?;
//     log::info!("3");
//     main_loop(connection, initialization_params)?;
//     log::info!("4");
//     io_threads.join()?;
//     log::info!("5");

//     // Shut down gracefully.
//     log::info!("shutting down server");
//     Ok(())
// }

// fn main_loop(
//     connection: Connection,
//     params: serde_json::Value,
// ) -> Result<(), Box<dyn Error + Sync + Send>> {
//     // params.as_object_mut()
//     log::info!("blub");
//     let _params: InitializeParams = serde_json::from_value(params).unwrap();
//     log::info!("starting example main loop");
//     for msg in &connection.receiver {
//         log::info!("got msg: {:?}", msg);
//         match msg {
//             Message::Request(req) => {
//                 if connection.handle_shutdown(&req)? {
//                     return Ok(());
//                 }
//                 log::info!("got request: {:?}", req);
//                 match cast::<GotoDefinition>(req) {
//                     Ok((id, params)) => {
//                         log::info!("got gotoDefinition request #{}: {:?}", id, params);
//                         let result = Some(GotoDefinitionResponse::Array(Vec::new()));
//                         let result = serde_json::to_value(&result).unwrap();
//                         let resp = Response {
//                             id,
//                             result: Some(result),
//                             error: None,
//                         };
//                         connection.sender.send(Message::Response(resp))?;
//                         continue;
//                     }
//                     Err(req) => req,
//                 };
//                 // ...
//             }
//             Message::Response(resp) => {
//                 log::info!("got response: {:?}", resp);
//             }
//             Message::Notification(not) => {
//                 log::info!("got notification: {:?}", not);
//             }
//         }
//     }
//     Ok(())
// }

// fn cast<R>(req: Request) -> Result<(RequestId, R::Params), Request>
// where
//     R: lsp_types::request::Request,
//     R::Params: serde::de::DeserializeOwned,
// {
//     req.extract(R::METHOD)
// }
