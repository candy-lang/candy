use crate::{client::StdoutWriter, requests::Request, responses::Response};
use async_trait::async_trait;

/// Trait for an debug adapter.
///
/// Adapters are the main backbone of a debug server. They get a `accept` call for each
/// incoming request. Responses are the return values of these calls.
#[async_trait]
pub trait Adapter {
    /// Accept (and take ownership) of an incoming request.
    ///
    /// This is the primary entry point for debug adapters, where deserialized requests
    /// can be processed.
    ///
    /// The `ctx` reference can be used to send events and reverse requests to the client.
    ///
    /// # Error handling
    ///
    /// This function always returns a valid `Response` object,  however, that response
    /// itself may be an error response. As such, implementors should map their errors to
    /// an error response to allow clients to handle them. This is in the interest of users -
    /// the debug adapter is not something that users directly interact with nor something
    /// that they necessarily know about. From the users' perspective, it's an implementation
    /// detail and they are using their editor to debug something.
    async fn handle_request(
        &mut self,
        request: Request,
        stdout_writer: &mut StdoutWriter,
    ) -> Response;
}
