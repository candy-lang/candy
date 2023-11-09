use crate::{
    byte_code::ByteCode,
    heap::{Data, Handle, Heap, InlineObject, Int, List, Struct, Tag, Text},
    tracer::Tracer,
    vm::VmHandleCall,
    StateAfterRun, StateAfterRunForever, Vm, VmFinished,
};
use candy_frontend::utils::HashMapExtension;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    borrow::{Borrow, Cow},
    io::{self, BufRead},
    net::SocketAddr,
    str::FromStr,
};
use tiny_http::{Request, Response, Server};
use tracing::info;

pub trait Environment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<B, T>,
    ) -> Vm<B, T>;
}

pub struct EmptyEnvironment;
impl Environment for EmptyEnvironment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        _heap: &mut Heap,
        _call: VmHandleCall<B, T>,
    ) -> Vm<B, T> {
        panic!("A handle was called.")
    }
}
impl<B: Borrow<ByteCode>, T: Tracer> Vm<B, T> {
    pub fn run_without_handles(self, heap: &mut Heap) -> StateAfterRunWithoutHandles<B, T> {
        self.run_with_environment(heap, &mut EmptyEnvironment)
    }
    pub fn run_n_without_handles(
        self,
        heap: &mut Heap,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<B, T> {
        self.run_n_with_environment(heap, &mut EmptyEnvironment, max_instructions)
    }
    pub fn run_forever_without_handles(self, heap: &mut Heap) -> VmFinished<T> {
        self.run_forever_with_environment(heap, &mut EmptyEnvironment)
    }
}

pub struct DefaultEnvironment {
    get_random_bytes_handle: Handle,

    http_server_handle: Handle,
    /// `None` means the server got closed.
    http_server_states: Vec<Option<HttpServerState>>,

    stdin_handle: Handle,
    stdout_handle: Handle,

    dynamic_handles: FxHashMap<Handle, DynamicHandle>,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(clippy::enum_variant_names)]
enum DynamicHandle {
    HttpServerGetNextRequest(HttpServerIndex),
    HttpServerSendResponse(HttpServerIndex, HttpRequestId),
    HttpServerClose(HttpServerIndex),
}
struct HttpServerState {
    server: Server,
    next_request_id: HttpRequestId,
    open_requests: FxHashMap<HttpRequestId, Request>,
}
type HttpServerIndex = usize;
type HttpRequestId = usize;

impl DefaultEnvironment {
    pub fn new(heap: &mut Heap, args: &[String]) -> (Struct, Self) {
        let arguments = args
            .iter()
            .map(|it| Text::create(heap, true, it).into())
            .collect_vec();
        let arguments = List::create(heap, true, arguments.as_slice());
        let get_random_bytes_handle = Handle::new(heap, 1);
        let http_server_handle = Handle::new(heap, 0);
        let stdin_handle = Handle::new(heap, 0);
        let stdout_handle = Handle::new(heap, 1);
        let environment_object = Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (heap.default_symbols().arguments, arguments.into()),
                (
                    heap.default_symbols().get_random_bytes,
                    **get_random_bytes_handle,
                ),
                (heap.default_symbols().http_server, **http_server_handle),
                (heap.default_symbols().stdin, **stdin_handle),
                (heap.default_symbols().stdout, **stdout_handle),
            ],
        );
        let environment = Self {
            get_random_bytes_handle,
            http_server_handle,
            http_server_states: vec![],
            stdin_handle,
            stdout_handle,
            dynamic_handles: FxHashMap::default(),
        };
        (environment_object, environment)
    }
}
impl Environment for DefaultEnvironment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<B, T>,
    ) -> Vm<B, T> {
        let result = if call.handle == self.get_random_bytes_handle {
            Self::get_random_bytes(heap, &call.arguments)
        } else if call.handle == self.http_server_handle {
            self.http_server(heap, &call.arguments)
        } else if call.handle == self.stdin_handle {
            Self::stdin(heap, &call.arguments)
        } else if call.handle == self.stdout_handle {
            Self::stdout(heap, &call.arguments)
        } else {
            let dynamic_handle = self.dynamic_handles.get(&call.handle).unwrap_or_else(|| {
                panic!(
                    "A handle was called that doesn't exist: {handle:?}",
                    handle = call.handle
                )
            });
            match dynamic_handle {
                DynamicHandle::HttpServerGetNextRequest(server_index) => {
                    self.http_server_get_next_request(heap, *server_index, &call.arguments)
                }
                DynamicHandle::HttpServerSendResponse(server_index, request_index) => self
                    .http_server_send_response(
                        heap,
                        *server_index,
                        *request_index,
                        &call.arguments,
                    ),
                DynamicHandle::HttpServerClose(server_index) => {
                    self.http_server_close(heap, *server_index, &call.arguments)
                }
            }
        };
        call.complete(heap, result)
    }
}
impl DefaultEnvironment {
    fn get_random_bytes(heap: &mut Heap, arguments: &[InlineObject]) -> InlineObject {
        let [length] = arguments else { unreachable!() };
        let Data::Int(length) = (*length).into() else {
            // TODO: Panic
            let message = Text::create(
                heap,
                true,
                "Handle `getRandomBytes` was called with a non-integer.",
            );
            return Tag::create_result(heap, true, Err(message.into())).into();
        };
        let Some(length) = length.try_get::<usize>() else {
            // TODO: Panic
            let message = Text::create(
                heap,
                true,
                "Handle `getRandomBytes` was called with a length that doesn't fit in usize.",
            );
            return Tag::create_result(heap, true, Err(message.into())).into();
        };

        let mut bytes = vec![0u8; length];
        if let Err(error) = getrandom::getrandom(&mut bytes) {
            let message = Text::create(heap, true, &error.to_string());
            return Tag::create_result(heap, true, Err(message.into())).into();
        }

        let bytes = bytes
            .into_iter()
            .map(|it| Int::create(heap, true, it).into())
            .collect_vec();
        let bytes = List::create(heap, true, bytes.as_slice());
        Tag::create_result(heap, true, Ok(bytes.into())).into()
    }

    fn http_server(&mut self, heap: &mut Heap, arguments: &[InlineObject]) -> InlineObject {
        let [list_of_socket_texts] = arguments else {
            unreachable!()
        };

        let Data::List(list_of_socket_texts) = (*list_of_socket_texts).into() else {
            // TODO: Panic
            let message = Text::create(
                heap,
                true,
                "Handle `httpServer` was called with a non-list.",
            );
            return Tag::create_result(heap, true, Err(message.into())).into();
        };
        let list_of_socket_addresses: Vec<_> = match list_of_socket_texts
            .items()
            .iter()
            .map(|it| {
                let Data::Text(text) = (*it).into() else {
                    return Err(Cow::Borrowed(
                        "Handle `httpServer` was called with a list containing non-texts.",
                    ));
                };
                match SocketAddr::from_str(text.get()) {
                    Ok(address) => Ok(address),
                    Err(error) => Err(Cow::Owned(format!(
                        "Handle `httpServer` was called with an invalid socket address: {error}"
                    ))),
                }
            })
            .collect()
        {
            Ok(list_of_socket_addresses) => list_of_socket_addresses,
            Err(error_message) => {
                // TODO: Panic
                let message = Text::create(heap, true, error_message.borrow());
                return Tag::create_result(heap, true, Err(message.into())).into();
            }
        };

        let server = match Server::http(list_of_socket_addresses.as_slice()) {
            Ok(server) => server,
            Err(error) => {
                let message = Text::create(heap, true, &error.to_string());
                return Tag::create_result(heap, true, Err(message.into())).into();
            }
        };

        let server_index = self.http_server_states.len();
        self.http_server_states
            .push(Some(HttpServerState::new(server)));

        let get_next_request_handle = self.create_dynamic_handle(
            heap,
            DynamicHandle::HttpServerGetNextRequest(server_index),
            0,
        );
        let close_handle =
            self.create_dynamic_handle(heap, DynamicHandle::HttpServerClose(server_index), 0);
        Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (
                    heap.default_symbols().get_next_request,
                    **get_next_request_handle,
                ),
                (heap.default_symbols().close, **close_handle),
            ],
        )
        .into()
    }
    fn http_server_get_next_request(
        &mut self,
        heap: &mut Heap,
        server_index: HttpServerIndex,
        arguments: &[InlineObject],
    ) -> InlineObject {
        assert!(arguments.is_empty());

        let server_state = &mut self.http_server_states[server_index];
        let Some(server_state) = server_state else {
            // TODO: Panic
            return Self::http_server_error_closed(heap);
        };

        let mut request = match server_state.server.recv() {
            Ok(request) => request,
            Err(error) => {
                let message = Text::create(heap, true, &error.to_string());
                return Tag::create_result(heap, true, Err(message.into())).into();
            }
        };

        // TODO: Support binary request bodies and other encodings
        let mut body = String::new();
        if let Err(error) = request.as_reader().read_to_string(&mut body) {
            let message = Text::create(heap, true, &error.to_string());
            return Tag::create_result(heap, true, Err(message.into())).into();
        }
        // TODO: Expose all request properties, not just the body
        let request_text = Text::create(heap, true, &body);

        let request_id = server_state.next_request_id;
        server_state.next_request_id += 1;
        server_state.open_requests.force_insert(request_id, request);

        let send_response_handle = self.create_dynamic_handle(
            heap,
            DynamicHandle::HttpServerSendResponse(server_index, request_id),
            1,
        );

        let result = Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (heap.default_symbols().request, request_text.into()),
                (heap.default_symbols().send_response, **send_response_handle),
            ],
        );
        Tag::create_result(heap, true, Ok(result.into())).into()
    }
    fn http_server_send_response(
        &mut self,
        heap: &mut Heap,
        server_index: HttpServerIndex,
        request_id: HttpRequestId,
        arguments: &[InlineObject],
    ) -> InlineObject {
        let [body] = arguments else {
            unreachable!();
        };

        let Data::Text(body) = (*body).into() else {
            // TODO: Panic
            let message = Text::create(
                heap,
                true,
                "Handle `httpRequest.sendResponse` was called with a non-text.",
            );
            return Tag::create_result(heap, true, Err(message.into())).into();
        };

        let server_state = &mut self.http_server_states[server_index];
        let Some(server_state) = server_state else {
            // TODO: Panic
            return Self::http_server_error_closed(heap);
        };

        let request = server_state.open_requests.remove(&request_id);
        let Some(request) = request else {
            // TODO: Panic
            let message = Text::create(
                heap,
                true,
                "Handle `httpRequest.sendResponse` was called for a request that was already responded to.",
            );
            return Tag::create_result(heap, true, Err(message.into())).into();
        };

        // TODO: Support all response properties, not just the body.
        let response = Response::from_string(body.get());
        let result = match request.respond(response) {
            Ok(()) => Ok(Tag::create_nothing(heap).into()),
            Err(error) => Err(Text::create(heap, true, &error.to_string()).into()),
        };
        Tag::create_result(heap, true, result).into()
    }
    fn http_server_close(
        &mut self,
        heap: &mut Heap,
        server_index: HttpServerIndex,
        arguments: &[InlineObject],
    ) -> InlineObject {
        assert!(arguments.is_empty());

        let server_state = &mut self.http_server_states[server_index];
        if server_state.is_none() {
            // TODO: Panic
            return Self::http_server_error_closed(heap);
        }

        // The server is closed when dropped.
        *server_state = None;

        Tag::create_nothing(heap).into()
    }
    fn http_server_error_closed(heap: &mut Heap) -> InlineObject {
        let message = Text::create(heap, true, "The HTTP server was closed already.");
        Tag::create_result(heap, true, Err(message.into())).into()
    }

    fn stdin(heap: &mut Heap, arguments: &[InlineObject]) -> InlineObject {
        assert!(arguments.is_empty());
        let input = {
            let stdin = io::stdin();
            stdin.lock().lines().next().unwrap().unwrap()
        };
        Text::create(heap, true, &input).into()
    }
    fn stdout(heap: &Heap, arguments: &[InlineObject]) -> InlineObject {
        let [message] = arguments else { unreachable!() };
        if let Data::Text(text) = (*message).into() {
            println!("{}", text.get());
        } else {
            info!("Non-text value sent to stdout: {message:?}");
        }

        Tag::create_nothing(heap).into()
    }

    fn create_dynamic_handle(
        &mut self,
        heap: &mut Heap,
        dynamic_handle: DynamicHandle,
        argument_count: usize,
    ) -> Handle {
        let handle = Handle::new(heap, argument_count);
        self.dynamic_handles.force_insert(handle, dynamic_handle);
        handle
    }
}

impl HttpServerState {
    fn new(server: Server) -> Self {
        Self {
            server,
            next_request_id: 0,
            open_requests: FxHashMap::default(),
        }
    }
}

#[must_use]
pub enum StateAfterRunWithoutHandles<B: Borrow<ByteCode>, T: Tracer> {
    Running(Vm<B, T>),
    Finished(VmFinished<T>),
}
impl<B: Borrow<ByteCode>, T: Tracer> Vm<B, T> {
    pub fn run_with_environment(
        self,
        heap: &mut Heap,
        environment: &mut impl Environment,
    ) -> StateAfterRunWithoutHandles<B, T> {
        match self.run(heap) {
            StateAfterRun::Running(vm) => StateAfterRunWithoutHandles::Running(vm),
            StateAfterRun::CallingHandle(call) => {
                StateAfterRunWithoutHandles::Running(environment.handle(heap, call))
            }
            StateAfterRun::Finished(finished) => StateAfterRunWithoutHandles::Finished(finished),
        }
    }

    pub fn run_n_with_environment(
        mut self,
        heap: &mut Heap,
        environment: &mut impl Environment,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<B, T> {
        for _ in 0..max_instructions {
            match self.run_with_environment(heap, environment) {
                StateAfterRunWithoutHandles::Running(vm) => self = vm,
                finished @ StateAfterRunWithoutHandles::Finished(_) => return finished,
            }
        }
        StateAfterRunWithoutHandles::Running(self)
    }

    pub fn run_forever_with_environment(
        mut self,
        heap: &mut Heap,
        environment: &mut impl Environment,
    ) -> VmFinished<T> {
        loop {
            match self.run_forever(heap) {
                StateAfterRunForever::CallingHandle(call) => self = environment.handle(heap, call),
                StateAfterRunForever::Finished(finished) => return finished,
            }
        }
    }
}
