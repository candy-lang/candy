use super::SessionId;
use dap::{
    prelude::{Event, EventBody},
    requests::{Command, Request},
    responses::{Response, ResponseBody},
    types::Capabilities,
};
use lsp_types::notification::Notification;
use serde::{Deserialize, Serialize};
use tower_lsp::Client;

#[derive(Debug)]
pub struct DebugAdapter {
    pub session_id: SessionId,
    pub client: Client,
}

impl DebugAdapter {
    pub async fn handle(&mut self, request: Request) {
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
                self.send_response(request.seq, ResponseBody::Initialize(Some(capabilities)))
                    .await;
                self.send_event(EventBody::Initialized).await;
            }
            Command::Launch(_) => todo!(),
            Command::LoadedSources => todo!(),
            Command::Modules(_) => todo!(),
            Command::Next(_) => todo!(),
            Command::Pause(_) => todo!(),
            Command::ReadMemory(_) => todo!(),
            Command::Restart(_) => todo!(),
            Command::RestartFrame(_) => todo!(),
            Command::ReverseContinue(_) => todo!(),
            Command::Scopes(_) => todo!(),
            Command::SetBreakpoints(_) => todo!(),
            Command::SetDataBreakpoints(_) => todo!(),
            Command::SetExceptionBreakpoints(_) => todo!(),
            Command::SetExpression(_) => todo!(),
            Command::SetFunctionBreakpoints(_) => todo!(),
            Command::SetInstructionBreakpoints(_) => todo!(),
            Command::SetVariable(_) => todo!(),
            Command::Source(_) => todo!(),
            Command::StackTrace(_) => todo!(),
            Command::StepBack(_) => todo!(),
            Command::StepIn(_) => todo!(),
            Command::StepInTargets(_) => todo!(),
            Command::StepOut(_) => todo!(),
            Command::Terminate(_) => todo!(),
            Command::TerminateThreads(_) => todo!(),
            Command::Threads => todo!(),
            Command::Variables(_) => todo!(),
            Command::WriteMemory(_) => todo!(),
            Command::Cancel(_) => todo!(),
        }
    }

    async fn send_response(&self, seq: i64, body: ResponseBody) {
        self.client
            .send_notification::<ResponseNotification>(ResponseNotification {
                session_id: self.session_id.to_owned(),
                message: Response {
                    request_seq: seq,
                    success: true,
                    message: None,
                    body: Some(body),
                },
            })
            .await;
    }
    async fn send_event(&self, body: EventBody) {
        self.client
            .send_notification::<EventNotification>(EventNotification {
                session_id: self.session_id.to_owned(),
                message: Event::make_event(body),
            })
            .await;
    }
}

// Response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseNotification {
    pub session_id: SessionId,
    pub message: Response,
}
impl Notification for ResponseNotification {
    const METHOD: &'static str = "candy/debugAdapter/message";

    type Params = Self;
}
// Even though we only ever send this notification, `tower_lsp` still requires it to be deserializeable.
impl<'de> Deserialize<'de> for ResponseNotification {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        panic!("ResponseNotification is not deserializable.")
    }
}

// Event
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventNotification {
    pub session_id: SessionId,
    pub message: Event,
}
impl Notification for EventNotification {
    const METHOD: &'static str = "candy/debugAdapter/message";

    type Params = Self;
}
// Even though we only ever send this notification, `tower_lsp` still requires it to be deserializeable.
impl<'de> Deserialize<'de> for EventNotification {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        panic!("EventNotification is not deserializable.")
    }
}
