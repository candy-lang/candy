use super::{tracer::DebugTracer, ServerToClient, ServerToClientMessage, SessionId};
use crate::database::Database;
use candy_frontend::{
    hir::Id,
    id::CountableId,
    module::{Module, ModuleKind, PackagesPath},
    utils::AdjustCasingOfFirstLetter,
    TracingConfig, TracingMode,
};
use candy_vm::{
    context::{DbUseProvider, RunLimitedNumberOfInstructions},
    fiber::FiberId,
    heap::{Data, Struct},
    mir_to_lir::MirToLir,
    run_lir,
    tracer::DummyTracer,
    vm::{FiberTree, Vm},
};
use dap::{
    events::StoppedEventBody,
    prelude::EventBody,
    requests::{Command, Request},
    responses::{
        Response, ResponseBody, ResponseMessage, ScopesResponse, SetExceptionBreakpointsResponse,
        StackTraceResponse, ThreadsResponse,
    },
    types::{Capabilities, StackFrame, StackFramePresentationhint, StoppedEventReason, Thread},
};
use rustc_hash::FxHashMap;
use std::{hash::Hash, panic, path::PathBuf};
use tokio::sync::mpsc;
use tower_lsp::Client;
use tracing::error;

#[tokio::main(worker_threads = 1)]
pub async fn run_debug_session(
    session_id: SessionId,
    client: Client,
    packages_path: PackagesPath,
    mut client_to_server: mpsc::Receiver<Request>,
) {
    // TODO: Share database with language server.
    let db = Database::new_with_file_system_module_provider(packages_path);
    let mut session = DebugSession {
        session_id,
        client,
        db,
        state: State::Initial,
    };
    while let Some(request) = client_to_server.recv().await {
        let seq = request.seq;
        match session.handle(request).await {
            Ok(()) => {}
            Err(message) => {
                session
                    .send_response_err(seq, ResponseMessage::Error(message.to_string()))
                    .await
            }
        }
    }
}

struct DebugSession {
    session_id: SessionId,
    client: Client,
    db: Database,
    state: State,
}

enum State {
    Initial,
    Running(VmState),
    Paused(PausedState),
}
struct PausedState {
    vm_state: VmState,
    stack_frame_ids: IdMapping<(FiberId, usize)>,
}
struct VmState {
    vm: Vm,
    tracer: DebugTracer,
}

impl DebugSession {
    pub async fn handle(&mut self, request: Request) -> Result<(), &'static str> {
        match request.command {
            Command::Attach(_) => todo!(),
            Command::BreakpointLocations(_) => todo!(),
            Command::Completions(_) => todo!(),
            Command::ConfigurationDone => todo!(),
            Command::Continue(_) => todo!(),
            Command::DataBreakpointInfo(_) => todo!(),
            Command::Disassamble(_) => todo!(),
            Command::Disconnect(_) => todo!(),
            Command::Evaluate(_) => todo!(),
            Command::ExceptionInfo(_) => todo!(),
            Command::Goto(_) => todo!(),
            Command::GotoTargets(_) => todo!(),
            Command::Initialize(_) => {
                if !matches!(self.state, State::Initial) {
                    return Err("already-initialized");
                }

                let capabilities = Capabilities {
                    supports_configuration_done_request: None,
                    supports_function_breakpoints: None,
                    supports_conditional_breakpoints: None,
                    supports_hit_conditional_breakpoints: None,
                    supports_evaluate_for_hovers: None,
                    exception_breakpoint_filters: None,
                    supports_step_back: None,
                    supports_set_variable: None,
                    supports_restart_frame: None,
                    supports_goto_targets_request: None,
                    supports_step_in_targets_request: None,
                    supports_completions_request: None,
                    completion_trigger_characters: None,
                    supports_modules_request: None,
                    additional_module_columns: None,
                    supported_checksum_algorithms: None,
                    supports_restart_request: None,
                    supports_exception_options: None,
                    supports_value_formatting_options: None,
                    supports_exception_info_request: None,
                    support_terminate_debuggee: None,
                    support_suspend_debuggee: None,
                    supports_delayed_stack_trace_loading: None,
                    supports_loaded_sources_request: None,
                    supports_log_points: None,
                    supports_terminate_threads_request: None,
                    supports_set_expression: None,
                    supports_terminate_request: None,
                    supports_data_breakpoints: None,
                    supports_read_memory_request: None,
                    supports_write_memory_request: None,
                    supports_disassemble_request: None,
                    supports_cancel_request: None,
                    supports_breakpoint_locations_request: None,
                    supports_clipboard_context: None,
                    supports_stepping_granularity: None,
                    supports_instruction_breakpoints: None,
                    supports_exception_filter_options: None,
                    supports_single_thread_execution_requests: None,
                };
                self.send_response_ok(request.seq, ResponseBody::Initialize(Some(capabilities)))
                    .await;
                self.send(EventBody::Initialized).await;
                Ok(())
            }
            Command::Launch(args) => {
                if !matches!(self.state, State::Initial) {
                    return Err("already-launched");
                }

                let module = self.parse_module(args.program)?;

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::Off,
                    calls: TracingMode::All,
                    evaluated_expressions: TracingMode::Off,
                };
                let lir = self
                    .db
                    .lir(module.clone(), tracing.clone())
                    .unwrap()
                    .as_ref()
                    .to_owned();
                let use_provider = DbUseProvider {
                    db: &self.db,
                    tracing,
                };
                let mut tracer = DummyTracer::default();
                let (mut heap, main) =
                    match run_lir(module, lir, &use_provider, &mut tracer).into_main_function() {
                        Ok(result) => result,
                        Err(error) => {
                            error!("Failed to find main function: {error}");
                            return Err("program-invalid");
                        }
                    };

                let mut vm = Vm::default();
                self.send_response_ok(request.seq, ResponseBody::Launch)
                    .await;

                // Run the `main` function.
                let environment = Struct::create(&mut heap, &FxHashMap::default());
                vm.set_up_for_running_closure(heap, main, &[environment.into()], Id::platform());

                let mut execution_controller = RunLimitedNumberOfInstructions::new(10000);
                let mut tracer = DebugTracer::default();
                // FIXME: remove
                vm.run(&use_provider, &mut execution_controller, &mut tracer);

                self.state = State::Paused(PausedState {
                    vm_state: VmState { vm, tracer },
                    stack_frame_ids: IdMapping::default(),
                });

                self.send(EventBody::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Entry,
                    description: Some("Paused on program start".to_string()),
                    thread_id: Some(Self::fiber_id_to_thread_id(FiberId::root())),
                    preserve_focus_hint: Some(false),
                    text: None,
                    all_threads_stopped: Some(true),
                    hit_breakpoint_ids: Some(vec![]),
                }))
                .await;

                Ok(())
            }
            Command::LoadedSources => todo!(),
            Command::Modules(_) => todo!(),
            Command::Next(_) => todo!(),
            Command::Pause(_) => todo!(),
            Command::ReadMemory(_) => todo!(),
            Command::Restart(_) => todo!(),
            Command::RestartFrame(_) => todo!(),
            Command::ReverseContinue(_) => todo!(),
            Command::Scopes(_) => {
                self.send_response_ok(
                    request.seq,
                    // FIXME: implement
                    ResponseBody::Scopes(ScopesResponse { scopes: vec![] }),
                )
                .await;
                Ok(())
            }
            Command::SetBreakpoints(_) => todo!(),
            Command::SetDataBreakpoints(_) => todo!(),
            Command::SetExceptionBreakpoints(_) => {
                self.send_response_ok(
                    request.seq,
                    ResponseBody::SetExceptionBreakpoints(Some(SetExceptionBreakpointsResponse {
                        breakpoints: Some(vec![]),
                    })),
                )
                .await;
                Ok(())
            }
            Command::SetExpression(_) => todo!(),
            Command::SetFunctionBreakpoints(_) => todo!(),
            Command::SetInstructionBreakpoints(_) => todo!(),
            Command::SetVariable(_) => todo!(),
            Command::Source(_) => todo!(),
            Command::StackTrace(args) => {
                let state = self.require_paused_mut()?;

                let fiber_id = Self::thread_id_to_fiber_id(args.thread_id);
                let fiber_state = state
                    .vm_state
                    .tracer
                    .fibers
                    .get(&fiber_id)
                    .ok_or("fiber-not-found")?;

                let start_frame = args.start_frame.map(|it| it as usize).unwrap_or_default();
                let mut call_stack = &fiber_state.call_stack[start_frame..];
                if let Some(levels) = args.levels {
                    let levels = levels as usize;
                    if levels < call_stack.len() {
                        call_stack = &call_stack[..levels];
                    }
                }

                let stack_frames = call_stack
                    .iter()
                    .enumerate()
                    .map(|(index, it)| {
                        // TODO: format arguments
                        let name = match Data::from(it.callee) {
                            // TODO: resolve function name
                            Data::Closure(closure) => format!("{:p}", closure.address()),
                            Data::Builtin(builtin) => format!(
                                "✨.{}",
                                format!("{:?}", builtin.get()).lowercase_first_letter(),
                            ),
                            Data::Tag(tag) => tag.symbol().get().to_owned(),
                            it => panic!("Unexpected callee: {it}"),
                        };
                        StackFrame {
                            id: state.stack_frame_ids.get((fiber_id, index)),
                            name,
                            source: None,
                            line: 1,
                            column: 1,
                            end_line: None,
                            end_column: None,
                            can_restart: Some(false),
                            instruction_pointer_reference: None,
                            module_id: None,
                            presentation_hint: Some(StackFramePresentationhint::Normal),
                        }
                    })
                    .collect();
                let total_frames = fiber_state.call_stack.len() as i64;
                self.send_response_ok(
                    request.seq,
                    ResponseBody::StackTrace(StackTraceResponse {
                        stack_frames,
                        total_frames: Some(total_frames),
                    }),
                )
                .await;
                Ok(())
            }
            Command::StepBack(_) => todo!(),
            Command::StepIn(_) => todo!(),
            Command::StepInTargets(_) => todo!(),
            Command::StepOut(_) => todo!(),
            Command::Terminate(_) => todo!(),
            Command::TerminateThreads(_) => todo!(),
            Command::Threads => {
                let state = self.require_launched()?;

                dbg!(&state.tracer);

                self.send_response_ok(
                    request.seq,
                    ResponseBody::Threads(ThreadsResponse {
                        threads: state
                            .vm
                            .fibers()
                            .iter()
                            .map(|(id, fiber)| Thread {
                                // FIXME: Use data from tracer?
                                id: Self::fiber_id_to_thread_id(*id),
                                name: format!(
                                    "Fiber {}{}",
                                    id.to_usize(),
                                    match fiber {
                                        FiberTree::Single(_) => "",
                                        FiberTree::Parallel(_) => " (in `parallel`)",
                                        FiberTree::Try(_) => " (in `try`)",
                                    },
                                ),
                            })
                            .collect(),
                    }),
                )
                .await;

                Ok(())
            }
            Command::Variables(_) => todo!(),
            Command::WriteMemory(_) => todo!(),
            Command::Cancel(_) => todo!(),
        }
    }

    fn parse_module(&self, path: Option<String>) -> Result<Module, &'static str> {
        let Some(path) = path else {
            error!("Missing program path");
            return Err("program-missing");
        };
        Module::from_path(
            &self.db.packages_path,
            &PathBuf::from(path),
            ModuleKind::Code,
        )
        .map_err(|err| {
            error!("Failed to find module: {err}");
            "program-invalid"
        })
    }

    fn require_launched(&self) -> Result<&VmState, &'static str> {
        match &self.state {
            State::Initial => Err("not-launched"),
            State::Running(state) => Ok(state),
            State::Paused(state) => Ok(&state.vm_state),
        }
    }
    fn require_paused(&self) -> Result<&PausedState, &'static str> {
        match &self.state {
            State::Paused(state) => Ok(state),
            _ => Err("not-paused"),
        }
    }
    fn require_paused_mut(&mut self) -> Result<&mut PausedState, &'static str> {
        match &mut self.state {
            State::Paused(state) => Ok(state),
            _ => Err("not-paused"),
        }
    }

    fn fiber_id_to_thread_id(id: FiberId) -> i64 {
        id.to_usize() as i64
    }
    fn thread_id_to_fiber_id(id: i64) -> FiberId {
        FiberId::from_usize(id as usize)
    }

    async fn send_response_ok(&self, seq: i64, body: ResponseBody) {
        self.send(Response {
            request_seq: seq,
            success: true,
            message: None,
            body: Some(body),
        })
        .await;
    }
    async fn send_response_err(&self, seq: i64, message: ResponseMessage) {
        self.send(Response {
            request_seq: seq,
            success: false,
            message: Some(message),
            body: None,
        })
        .await;
    }
    async fn send(&self, message: impl Into<ServerToClientMessage>) {
        let message = ServerToClient {
            session_id: self.session_id.to_owned(),
            message: message.into(),
        };
        self.client
            .send_notification::<ServerToClient>(message)
            .await;
    }
}

struct IdMapping<T: Eq + Hash> {
    next_id: i64,
    map: FxHashMap<T, i64>,
}
impl<T: Eq + Hash> IdMapping<T> {
    fn get(&mut self, key: T) -> i64 {
        *self.map.entry(key).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        })
    }
}
impl<T: Eq + Hash> Default for IdMapping<T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            map: Default::default(),
        }
    }
}
