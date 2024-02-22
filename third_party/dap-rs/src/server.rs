use std::fmt::Debug;

use crate::errors::{DeserializationError, ServerError};
use crate::line_reader::LineReader;
use crate::requests::Request;
use std::io::{Error as StdIoError, ErrorKind as StdIoErrorKind};
#[derive(Debug, Clone)]
enum ServerState {
    /// Expecting a header
    Header,
    /// Expecting a separator between header and content, i.e. "\r\n"
    Sep,
    /// Expecting content
    Content,
}

/// The `Server` is responsible for reading the incoming bytestream and constructing deserialized
/// requests from it. The main method of the `Server` is the `accept_request`
#[derive(Default)]
pub struct Server {}

fn escape_crlf(instr: &str) -> String {
    instr.replace('\n', "\\n").replace('\r', "\\r")
}

impl Server {
    /// Accept a single request from the `input` stream, convert it into `Request` and return it to the caller
    pub async fn accept_request(
        &mut self,
        input: &mut impl LineReader,
    ) -> Result<Request, ServerError> {
        let mut state = ServerState::Header;
        let mut content_length: usize = 0;

        loop {
            match state {
                ServerState::Header => {
                    let mut buffer = input.read_line().await?;

                    tracing::trace!("HEADER: read line: {}", escape_crlf(&buffer));
                    if buffer.is_empty() {
                        return Err(ServerError::IoError(StdIoError::new(
                            StdIoErrorKind::BrokenPipe,
                            "read an empty buffer",
                        )));
                    }

                    let parts: Vec<&str> = buffer.trim_end().split(':').collect();
                    if parts.len() == 2 {
                        match parts[0] {
                            "Content-Length" => {
                                content_length = match parts[1].trim().parse() {
                                    Ok(val) => val,
                                    Err(_) => {
                                        return Err(ServerError::HeaderParseError { line: buffer })
                                    }
                                };
                                buffer.clear();
                                buffer.reserve(content_length);
                                state = ServerState::Sep;
                            }
                            other => {
                                return Err(ServerError::UnknownHeader {
                                    header: other.to_string(),
                                })
                            }
                        }
                    } else if buffer.eq("\r\n") || buffer.eq("\n") {
                        tracing::trace!("HEADER: skipping empty line");
                        continue;
                    } else {
                        return Err(ServerError::HeaderParseError { line: buffer });
                    }
                }
                ServerState::Sep => {
                    let buffer = input.read_line().await?;

                    tracing::trace!("SEP: read line: {}", escape_crlf(&buffer));
                    if buffer == "\r\n" {
                        state = ServerState::Content;
                    } else {
                        // expecting separator
                        return Err(ServerError::ProtocolError {
                            reason: "failed to read separator".to_string(),
                        });
                    }
                }
                ServerState::Content => {
                    // read the payload
                    let mut payload = bytes::BytesMut::with_capacity(content_length);
                    let _ = input.read_n_bytes(&mut payload, content_length).await?;

                    let payload = String::from_utf8_lossy(&payload).to_string();
                    tracing::trace!("CONTENT: read content: {}", escape_crlf(&payload));
                    let request: Request = match serde_json::from_str(&payload) {
                        Ok(val) => val,
                        Err(e) => {
                            return Err(ServerError::ParseError(DeserializationError::SerdeError(
                                e,
                            )))
                        }
                    };
                    return Ok(request);
                }
            }
        }
    }
}
