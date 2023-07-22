use super::{
    paused::PausedState, tracer::DebugTracer, vm_state::VmState, ServerToClient,
    ServerToClientMessage, SessionId,
};
use crate::database::Database;
use candy_frontend::{
    hir::Id,
    id::CountableId,
    module::{Module, ModuleKind, PackagesPath},
    TracingConfig, TracingMode,
};
use candy_vm::{
    execution_controller::{ExecutionController, RunLimitedNumberOfInstructions},
    fiber::{Fiber, FiberId, InstructionPointer},
    heap::{HirId, Struct},
    lir::{Instruction, Lir},
    mir_to_lir::compile_lir,
    tracer::{DummyTracer, FiberTracer},
    vm::Vm,
};
use dap::{
    events::StoppedEventBody,
    prelude::EventBody,
    requests::{Command, InitializeArguments, Request},
    responses::{
        Response, ResponseBody, ResponseMessage, SetExceptionBreakpointsResponse, ThreadsResponse,
    },
    types::{Capabilities, StoppedEventReason},
};
use lsp_types::{Position, Range};
use rustc_hash::FxHashMap;
use std::{mem, num::NonZeroUsize, path::PathBuf, rc::Rc};
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

// `Launched` is much larger than `Initial` and `Initialized`, but it's also the
// most common state while the others are only temporary during initialization.
#[allow(clippy::large_enum_variant)]
enum State {
    Initial,
    Initialized(InitializeArguments),
    Launched {
        initialize_arguments: InitializeArguments,
        execution_state: ExecutionState,
    },
}

enum ExecutionState {
    #[allow(dead_code)] // WIP
    Running(VmState),
    Paused(PausedState),
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
            Command::Disconnect(_) => {
                let state = mem::replace(&mut self.state, State::Initial);
                let initialize_arguments = match state {
                    State::Initial | State::Initialized(_) => {
                        self.state = state;
                        return Err("not-launched");
                    }
                    State::Launched {
                        initialize_arguments,
                        ..
                    } => initialize_arguments,
                };
                self.state = State::Initialized(initialize_arguments);
                self.send_response_ok(request.seq, ResponseBody::Disconnect)
                    .await;
                Ok(())
            }
            Command::Evaluate(_) => todo!(),
            Command::ExceptionInfo(_) => todo!(),
            Command::Goto(_) => todo!(),
            Command::GotoTargets(_) => todo!(),
            Command::Initialize(args) => {
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
                    supports_read_memory_request: Some(true),
                    supports_write_memory_request: None,
                    supports_disassemble_request: None,
                    supports_cancel_request: None,
                    supports_breakpoint_locations_request: None,
                    supports_clipboard_context: None,
                    supports_stepping_granularity: None,
                    supports_instruction_breakpoints: None,
                    supports_exception_filter_options: None,
                    supports_single_thread_execution_requests: Some(true),
                };
                self.send_response_ok(request.seq, ResponseBody::Initialize(Some(capabilities)))
                    .await;
                self.send(EventBody::Initialized).await;
                self.state = State::Initialized(args);
                Ok(())
            }
            Command::Launch(args) => {
                let state = mem::replace(&mut self.state, State::Initial);
                let initialize_arguments = match state {
                    State::Initial => {
                        self.state = state;
                        return Err("not-initialized");
                    }
                    State::Initialized(initialize_arguments) => initialize_arguments,
                    State::Launched { .. } => {
                        self.state = state;
                        return Err("already-launched");
                    }
                };

                let module = self.parse_module(args.program)?;

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::Off,
                    calls: TracingMode::All,
                    evaluated_expressions: TracingMode::All,
                };
                let lir = compile_lir(&self.db, module.clone(), tracing.clone()).0;
                let (mut heap, main, constant_mapping) =
                    match Vm::for_module(&lir, &mut DummyTracer)
                        .run_until_completion(&mut DummyTracer)
                        .into_main_function()
                    {
                        Ok(result) => result,
                        Err(error) => {
                            error!("Failed to find main function: {error}");
                            return Err("program-invalid");
                        }
                    };

                let mut vm = Vm::uninitialized(Rc::new(lir));
                self.send_response_ok(request.seq, ResponseBody::Launch)
                    .await;

                // Run the `main` function.
                let environment = Struct::create(&mut heap, true, &FxHashMap::default()).into();
                let platform = HirId::create(&mut heap, true, Id::platform());
                let mut tracer = DebugTracer;
                vm.initialize_for_function(
                    heap,
                    constant_mapping,
                    main,
                    &[environment],
                    platform,
                    &mut tracer,
                );

                let mut execution_controller = RunLimitedNumberOfInstructions::new(10000);
                // TODO: remove when we support pause and continue
                vm.run(&mut execution_controller, &mut tracer);

                self.state = State::Launched {
                    initialize_arguments,
                    execution_state: ExecutionState::Paused(PausedState::new(VmState {
                        vm,
                        tracer,
                    })),
                };

                self.send(EventBody::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Entry,
                    description: Some("Paused on program start".to_string()),
                    thread_id: Some(FiberId::root().to_usize()),
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
            Command::Next(args) => {
                self.step(
                    request.seq,
                    StepKind::Next,
                    args.thread_id,
                    args.single_thread,
                )
                .await
            }
            Command::Pause(_) => todo!(),
            Command::ReadMemory(args) => {
                let state = self.state.require_paused_mut()?;
                let response = state.read_memory(args)?;
                self.send_response_ok(request.seq, ResponseBody::ReadMemory(Some(response)))
                    .await;
                Ok(())
            }
            Command::Restart(_) => todo!(),
            Command::RestartFrame(_) => todo!(),
            Command::ReverseContinue(_) => todo!(),
            Command::Scopes(args) => {
                let scopes = self.state.require_paused_mut()?.scopes(args);
                self.send_response_ok(request.seq, ResponseBody::Scopes(scopes))
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
                let start_at_1_config = self.state.require_initialized()?.into();
                let state = self.state.require_paused_mut()?;
                let stack_trace = state.stack_trace(&self.db, start_at_1_config, args)?;
                self.send_response_ok(request.seq, ResponseBody::StackTrace(stack_trace))
                    .await;
                Ok(())
            }
            Command::StepBack(_) => todo!(),
            Command::StepIn(args) => {
                self.step(
                    request.seq,
                    StepKind::In,
                    args.thread_id,
                    args.single_thread,
                )
                .await
            }
            Command::StepInTargets(_) => todo!(),
            Command::StepOut(args) => {
                self.step(
                    request.seq,
                    StepKind::Out,
                    args.thread_id,
                    args.single_thread,
                )
                .await
            }
            Command::Terminate(_) => todo!(),
            Command::TerminateThreads(_) => todo!(),
            Command::Threads => {
                let state = self.state.require_launched()?;
                let threads = state.threads();
                self.send_response_ok(
                    request.seq,
                    ResponseBody::Threads(ThreadsResponse { threads }),
                )
                .await;
                Ok(())
            }
            Command::Variables(args) => {
                let supports_variable_type = self
                    .state
                    .require_initialized()?
                    .supports_variable_type
                    .unwrap_or_default();
                let variables = self.state.require_paused_mut()?.variables(
                    &self.db,
                    args,
                    supports_variable_type,
                );
                self.send_response_ok(request.seq, ResponseBody::Variables(variables))
                    .await;
                Ok(())
            }
            Command::WriteMemory(_) => todo!(),
            Command::Cancel(_) => todo!(),
        }
    }
    async fn step(
        &mut self,
        request_seq: NonZeroUsize,
        kind: StepKind,
        thread_id: usize,
        single_thread: Option<bool>,
    ) -> Result<(), &'static str> {
        self.state.require_paused()?;
        let response_body = match kind {
            StepKind::Next => ResponseBody::Next,
            StepKind::In => ResponseBody::StepIn,
            StepKind::Out => ResponseBody::StepOut,
        };
        self.send_response_ok(request_seq, response_body).await;

        let state = self.state.require_paused_mut().unwrap();

        let fiber_id = FiberId::from_usize(thread_id);
        // TODO: honor `args.granularity`
        let fiber = state.vm_state.vm.fiber(fiber_id).unwrap().fiber_ref();
        let lir = state.vm_state.vm.lir().to_owned();
        let mut execution_controller =
            StepExecutionController::new(lir.as_ref(), fiber_id, fiber.call_stack().len(), kind);
        if single_thread.unwrap_or_default() {
            state.vm_state.vm.run_fiber(
                fiber_id,
                &mut execution_controller,
                &mut state.vm_state.tracer,
            );
        } else {
            state
                .vm_state
                .vm
                .run(&mut execution_controller, &mut state.vm_state.tracer);
        }

        self.send(EventBody::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Step,
            description: None,
            thread_id: Some(thread_id),
            preserve_focus_hint: Some(false),
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: Some(vec![]),
        }))
        .await;
        Ok(())
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

    async fn send_response_ok(&self, seq: NonZeroUsize, body: ResponseBody) {
        self.send(Response {
            request_seq: seq,
            success: true,
            message: None,
            body: Some(body),
        })
        .await;
    }
    async fn send_response_err(&self, seq: NonZeroUsize, message: ResponseMessage) {
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

impl State {
    fn require_initialized(&self) -> Result<&InitializeArguments, &'static str> {
        match &self {
            State::Initial => Err("not-initialized"),
            State::Initialized(initialize_arguments)
            | State::Launched {
                initialize_arguments,
                ..
            } => Ok(initialize_arguments),
        }
    }
    fn require_launched(&self) -> Result<&VmState, &'static str> {
        match &self {
            State::Initial | State::Initialized(_) => Err("not-launched"),
            State::Launched {
                execution_state:
                    ExecutionState::Running(vm_state)
                    | ExecutionState::Paused(PausedState { vm_state, .. }),
                ..
            } => Ok(vm_state),
        }
    }
    fn require_paused(&self) -> Result<&PausedState, &'static str> {
        match self {
            State::Launched {
                execution_state: ExecutionState::Paused(state),
                ..
            } => Ok(state),
            _ => Err("not-paused"),
        }
    }
    fn require_paused_mut(&mut self) -> Result<&mut PausedState, &'static str> {
        match self {
            State::Launched {
                execution_state: ExecutionState::Paused(state),
                ..
            } => Ok(state),
            _ => Err("not-paused"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StartAt1Config {
    lines_start_at_1: bool,
    columns_start_at_1: bool,
}
impl StartAt1Config {
    pub fn range_to_dap(&self, range: Range) -> Range {
        let start = self.position_to_dap(range.start);
        let end = self.position_to_dap(range.end);
        Range { start, end }
    }
    fn position_to_dap(&self, position: Position) -> Position {
        fn apply(start_at_1: bool, value: u32) -> u32 {
            if start_at_1 {
                value + 1
            } else {
                value
            }
        }
        Position {
            line: apply(self.lines_start_at_1, position.line),
            character: apply(self.columns_start_at_1, position.character),
        }
    }
}
impl From<&InitializeArguments> for StartAt1Config {
    fn from(value: &InitializeArguments) -> Self {
        Self {
            lines_start_at_1: value.lines_start_at1.unwrap_or(true),
            columns_start_at_1: value.columns_start_at1.unwrap_or(true),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StepKind {
    Next,
    In,
    Out,
}
struct StepExecutionController<'a> {
    lir: &'a Lir,
    fiber_id: FiberId,
    call_stack_size: usize,
    kind: StepKind,
    did_step: bool,
}
impl<'a> StepExecutionController<'a> {
    fn new(lir: &'a Lir, fiber_id: FiberId, call_stack_size: usize, kind: StepKind) -> Self {
        Self {
            lir,
            fiber_id,
            call_stack_size,
            kind,
            did_step: false,
        }
    }
}
impl<'a, T: FiberTracer> ExecutionController<T> for StepExecutionController<'a> {
    fn should_continue_running(&self) -> bool {
        !self.did_step
    }

    fn instruction_executed(
        &mut self,
        fiber_id: FiberId,
        fiber: &Fiber<T>,
        ip: InstructionPointer,
    ) {
        if fiber_id != self.fiber_id
            || !matches!(
                self.lir.instructions[*ip],
                Instruction::TraceCallEnds | Instruction::TraceExpressionEvaluated
            )
        {
            return;
        }

        self.did_step = match self.kind {
            StepKind::Next => fiber.call_stack().len() <= self.call_stack_size,
            StepKind::In => true,
            StepKind::Out => fiber.call_stack().len() < self.call_stack_size,
        }
    }
}
