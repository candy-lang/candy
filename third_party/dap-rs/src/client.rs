use serde::Serialize;

use crate::{
    errors::DeserializationError, events::Event, responses::Response,
    reverse_requests::ReverseRequest,
};

/// A simple writer to the stdout, used by the server to send replies back
/// to the IDE
#[derive(Clone, Debug, Default)]
pub struct StdoutWriter {
    should_exit: bool,
}

/// Trait for sending events and requests to the connected client.
impl StdoutWriter {
    /// Sends an even to the IDE.
    pub fn send_event(&mut self, event: Event) -> Result<(), DeserializationError> {
        self.write(Sendable::Event(event))
    }

    /// Sends a reverse request to the IDE.
    pub fn send_reverse_request(
        &mut self,
        request: ReverseRequest,
    ) -> Result<(), DeserializationError> {
        self.write(Sendable::ReverseRequest(request))
    }

    /// Sends a response to the IDE
    pub fn send_response(&mut self, response: Response) -> Result<(), DeserializationError> {
        self.write(Sendable::Response(response))
    }

    /// Notifies the server that it should gracefully exit after `accept`
    /// returned.
    ///
    /// It is recommended to send a `Terminated` and/or `Stopped` event to the client.
    pub fn request_exit(&mut self) {
        self.should_exit = true;
    }

    /// Clears an exit request set by `request_exit` in the same `accept` call.
    /// This cannot be used to clear an exit request that happened during a previous
    /// `accept`.
    pub fn cancel_exit(&mut self) {
        self.should_exit = false;
    }
    /// Returns `true` if the exiting was requested.
    pub fn get_exit_state(&self) -> bool {
        self.should_exit
    }

    pub fn write(&mut self, s: Sendable) -> Result<(), DeserializationError> {
        let resp_json = serde_json::to_string(&s)?;
        print!("Content-Length: {}\r\n\r\n", resp_json.len());
        print!("{}\r\n", resp_json);
        Ok(())
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Sendable {
    Response(Response),
    Event(Event),
    ReverseRequest(ReverseRequest),
}
