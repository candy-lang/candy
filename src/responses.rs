use serde::Serialize;

use crate::{
    errors::ServerError,
    requests::{Command, Request},
    types::{
        Breakpoint, BreakpointLocation, Capabilities, CompletionItem, DataBreakpointAccessType,
        DisassembledInstruction, ExceptionBreakMode, ExceptionDetails, GotoTarget, Module, Scope,
        Source, StackFrame, Thread, Variable, VariablePresentationHint,
    },
};

/// Represents a response messagte that is either a cancellation or a short error string.
#[derive(Serialize, Debug, Clone)]
pub enum ResponseMessage {
    /// Should be sent when the request was canceled
    Cancelled,
    /// Contains the raw error in short form if [`Response::success`](Response::success) is false.
    /// This raw error might be interpreted by the client and is not shown in the UI.
    Error(String),
}

#[derive(Serialize, Debug, Clone)]
pub struct BreakpointLocationsResponse {
    /// Sorted set of possible breakpoint locations.
    pub breakpoints: Vec<BreakpointLocation>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CompletionsResponse {
    /// The possible completions
    pub targets: Vec<CompletionItem>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ContinueResponse {
    /// The value true (or a missing property) signals to the client that all
    /// threads have been resumed. The value false indicates that not all threads
    /// were resumed.
    pub all_threads_continued: Option<bool>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DataBreakpointInfoResponse {
    /// An identifier for the data on which a data breakpoint can be registered
    /// with the `setDataBreakpoints` request or null if no data breakpoint is
    /// available.
    pub data_id: Option<String>,
    /// UI String that describes on what data the breakpoint is set on or why a
    /// data breakpoint is not available.
    pub description: String,
    /// Attribute lists the available access types for a potential data
    /// breakpoint. A UI client could surface this information.
    pub access_types: Option<Vec<DataBreakpointAccessType>>,
    /// Attribute indicates that a potential data breakpoint could be persisted
    /// across sessions.
    pub can_persist: Option<bool>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DisassembleResponse {
    /// The list of disassembled instructions.
    pub instructions: Vec<DisassembledInstruction>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
    /// The result of the evaluate request.
    pub result: String,
    /// The type of the evaluate result.
    /// This attribute should only be returned by a debug adapter if the
    /// corresponding capability `supportsVariableType` is true.
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    /// Properties of an evaluate result that can be used to determine how to
    /// render the result in the UI.
    pub presentation_hint: Option<VariablePresentationHint>,
    /// If `variablesReference` is > 0, the evaluate result is structured and its
    /// children can be retrieved by passing `variablesReference` to the
    /// `variables` request.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub variables_reference: i64,
    /// The i64 of named child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub named_variables: Option<i64>,
    /// The i64 of indexed child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub indexed_variables: Option<i64>,
    /// A memory reference to a location appropriate for this result.
    /// For pointer type eval results, this is generally a reference to the
    /// memory address contained in the pointer.
    /// This attribute should be returned by a debug adapter if corresponding
    /// capability `supportsMemoryReferences` is true.
    pub memory_reference: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionInfoResponse {
    /// ID of the exception that was thrown.
    pub exception_id: String,
    /// Descriptive text for the exception.
    pub description: Option<String>,
    /// Mode that caused the exception notification to be raised.
    pub break_mode: ExceptionBreakMode,
    /// Detailed information about the exception.
    pub details: Option<ExceptionDetails>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GotoTargetsResponse {
    /// The possible goto targets of the specified location.
    pub targets: Vec<GotoTarget>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoadedSourcesResponse {
    /// Set of loaded sources.
    pub sources: Vec<Source>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModulesResponse {
    /// All modules or range of modules.
    pub modules: Vec<Module>,
    /// The total i64 of modules available.
    pub total_modules: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReadMemoryResponse {
    /// The address of the first byte of data returned.
    /// Treated as a hex value if prefixed with `0x`, or as a decimal value
    /// otherwise.
    pub address: String,
    /// The i64 of unreadable bytes encountered after the last successfully
    /// read byte.
    /// This can be used to determine the i64 of bytes that should be skipped
    /// before a subsequent `readMemory` request succeeds.
    pub unreadable_bytes: Option<i64>,
    /// The bytes read from memory, encoded using base64.
    pub data: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScopesResponse {
    /// The scopes of the stackframe. If the array has length zero, there are no
    /// scopes available.
    pub scopes: Vec<Scope>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsResponse {
    /// Information about the breakpoints.
    /// The array elements are in the same order as the elements of the
    /// `breakpoints` (or the deprecated `lines`) array in the arguments.
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetDataBreakpointsResponse {
    /// Information about the breakpoints.
    /// The array elements are in the same order as the elements of the `breakpoints` array
    /// in the arguments.
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetExceptionBreakpointsResponse {
    /// Information about the exception breakpoints or filters.
    /// The breakpoints returned are in the same order as the elements of the
    /// `filters`, `filterOptions`, `exceptionOptions` arrays in the arguments.
    /// If both `filters` and `filterOptions` are given, the returned array must
    /// start with `filters` information first, followed by `filterOptions`
    /// information.
    pub breakpoints: Option<Vec<Breakpoint>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpointsResponse {
    /// Information about the breakpoints. The array elements correspond to the
    /// elements of the `breakpoints` array.
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetInstructionBreakpointsResponse {
    /// Information about the breakpoints. The array elements correspond to the
    /// elements of the `breakpoints` array.
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetVariableResponse {
    /// The new value of the variable.
    pub value: String,
    /// The type of the new value. Typically shown in the UI when hovering over
    /// the value.
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    /// If `variablesReference` is > 0, the new value is structured and its
    /// children can be retrieved by passing `variablesReference` to the
    /// `variables` request.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub variables_reference: Option<i64>,
    /// The i64 of named child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub named_variables: Option<i64>,
    /// The i64 of indexed child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub indexed_variables: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceResponse {
    /// Content of the source reference.
    pub content: String,
    /// Content type (MIME type) of the source.
    pub mime_type: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetExpressionResponse {
    /// The new value of the expression.
    pub value: String,
    /// The type of the value.
    /// This attribute should only be returned by a debug adapter if the
    /// corresponding capability `supportsVariableType` is true.
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    /// Properties of a value that can be used to determine how to render the
    /// result in the UI.
    pub presentation_hint: Option<VariablePresentationHint>,
    /// If `variablesReference` is > 0, the value is structured and its children
    /// can be retrieved by passing `variablesReference` to the `variables`
    /// request.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub variables_reference: Option<i64>,
    /// The i64 of named child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub named_variables: Option<i64>,
    /// The i64 of indexed child variables.
    /// The client can use this information to present the variables in a paged
    /// UI and fetch them in chunks.
    /// The value should be less than or equal to 2147483647 (2^31-1).
    pub indexed_variables: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponse {
    /// The frames of the stackframe. If the array has length zero, there are no
    /// stackframes available.
    /// This means that there is no location information available.
    pub stack_frames: Vec<StackFrame>,
    /// The total i64 of frames available in the stack. If omitted or if
    /// `totalFrames` is larger than the available frames, a client is expected
    /// to request frames until a request returns less frames than requested
    /// (which indicates the end of the stack). Returning monotonically
    /// increasing `totalFrames` values for subsequent requests can be used to
    /// enforce paging in the client.
    pub total_frames: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsResponse {
    /// All threads.
    pub threads: Vec<Thread>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VariablesResponse {
    /// All (or a range) of variables for the given variable reference.
    pub variables: Vec<Variable>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WriteMemoryResponse {
    /// Property that should be returned when `allowPartial` is true to indicate
    /// the offset of the first byte of data successfully written. Can be
    /// negative.
    pub offset: Option<i64>,
    /// Property that should be returned when `allowPartial` is true to indicate
    /// the i64 of bytes starting from address that were successfully written.
    pub bytes_written: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "command", content = "body", rename_all = "camelCase")]
pub enum ResponseBody {
    /// Response to attach request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Attach request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Attach)
    Attach,
    /// Response to breakpointLocations request.  Contains possible locations for source breakpoints.
    ///
    /// Specification: [BreakpointLocations request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_BreakpointLocations)
    BreakpointLocations(BreakpointLocationsResponse),
    /// Response to a `completions` request
    ///
    /// Specification: [Completions request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Completions)
    Completions(CompletionsResponse),
    /// Response to `configurationDone` request. This is just an acknowledgement, so no body field is
    /// required.
    ///
    /// Specification: [ConfigurationDone request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ConfigurationDone)
    ConfigurationDone,
    /// Response to `continue` request.
    ///
    /// Specification: [Continue request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Continue)
    Continue(ContinueResponse),
    /// Response to `dataBreakpointInfo` request.
    ///
    /// Specification: [DataBreakpointInfo request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_DataBreakpointInfo)
    DataBreakpointInfo(DataBreakpointInfoResponse),
    /// Response to `disassemble` request.
    ///
    /// Specification: [Disassembly request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Disassemble)
    Disassamble(Option<DisassembleResponse>),
    /// Response to disconnect request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Disconnect request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Disconnect)
    Disconnect,
    /// This is a special response that indicates no response being sent to the client.
    ///
    /// Note that this response variant is not part of the DAP specification, it only exists for
    /// the purposes of this crate.
    ///
    /// Normally, the adapter should send a response to all requests, so only use this in
    /// exceptional cases.
    Empty,
    /// Response to `evaluate` request.
    ///
    /// Specification: [Evaluate request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Evaluate)
    Evaluate(EvaluateResponse),
    /// Response to `exceptionInfo` request.
    ///
    /// Specification: [ExceptionInfo](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ExceptionInfo)
    ExceptionInfo(ExceptionInfoResponse),
    /// Response to `goto` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Goto request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Goto)
    Goto,
    /// Response to `gotoTargets` request.
    ///
    /// Specification: [GotoTargets request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_GotoTargets)
    GotoTargets(GotoTargetsResponse),
    /// Response to `initialize` request.
    ///
    /// Specification: [Initialize request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Initialize)
    Initialize(Option<Capabilities>),
    /// Response to launch request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Launch request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Launch)
    Launch,
    /// Response to `loadedSources` request.
    ///
    /// Specification: [LoadedSources request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_LoadedSources)
    LoadedSources(LoadedSourcesResponse),
    /// Response to `modules` request.
    ///
    /// Specification: [Modules request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Modules)
    Modules(ModulesResponse),
    /// Response to next request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Next request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Next)
    Next,
    /// Response to `pause` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Pause request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Pause)
    Pause,
    /// Response to readMemory request.
    ///
    /// Specification: [ReadMemory request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ReadMemory)
    ReadMemory(Option<ReadMemoryResponse>),
    /// Response to `restart` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Restart request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Restart)
    Restart,
    /// Response to `restartFrame` request. This is just an acknowledgement, so no body field is
    /// required.
    ///
    /// Specification: [RestartFrame request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_RestartFrame)
    RestartFrame,
    /// Response to `reverseContinue` request. This is just an acknowledgement, so no body field is
    /// required.
    ///
    /// Specification: [ReverseContinue request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ReverseContinue)
    ReverseContinue,
    /// Response to scopes request.
    ///
    /// Specification: [Scopes request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Scopes)
    Scopes(ScopesResponse),
    /// Response to setBreakpoints request.
    /// Returned is information about each breakpoint created by this request.
    /// This includes the actual code location and whether the breakpoint could be verified.
    /// The breakpoints returned are in the same order as the elements of the breakpoints
    /// (or the deprecated lines) array in the arguments.
    ///
    /// Specification: [SetBreakpointsRequest](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetBreakpoints)
    SetBreakpoints(SetBreakpointsResponse),
    /// Replaces all existing data breakpoints with new data breakpoints.
    /// To clear all data breakpoints, specify an empty array.
    /// When a data breakpoint is hit, a `stopped` event (with reason `date breakpoint`) is generated.
    /// Clients should only call this request if the corresponding capability
    /// `supportsDataBreakpoints` is true.
    ///
    /// Specification: [SetDataBreakpoints request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetDataBreakpoints)
    SetDataBreakpoints(SetDataBreakpointsResponse),
    /// Response to `setExceptionBreakpoint` request.
    ///
    /// The response contains an array of `Breakpoint` objects with information about each exception
    /// breakpoint or filter. The Breakpoint objects are in the same order as the elements of the
    /// `filters`, `filterOptions`, `exceptionOptions` arrays given as arguments. If both `filters`
    /// and `filterOptions` are given, the returned array must start with filters information first,
    /// followed by `filterOptions` information.
    ///
    /// The `verified` property of a `Breakpoint` object signals whether the exception breakpoint or
    /// filter could be successfully created and whether the condition or hit count expressions are
    /// valid. In case of an error the message property explains the problem. The id property can be
    /// used to introduce a unique ID for the exception breakpoint or filter so that it can be
    /// updated subsequently by sending breakpoint events.
    ///
    /// For backward compatibility both the `breakpoints` array and the enclosing body are optional.
    /// If these elements are missing a client is not able to show problems for individual exception
    /// breakpoints or filters.
    SetExceptionBreakpoints(Option<SetExceptionBreakpointsResponse>),
    /// Response to setExpression request.
    ///
    /// Specification: [SetExpression request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetExpression)
    SetExpression(SetExpressionResponse),
    /// Response to setFunctionBreakpoints request.
    /// Returned is information about each breakpoint created by this request.
    ///
    /// Specification: [SetFunctionBreakpointsArguments](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetFunctionBreakpoints)
    SetFunctionBreakpoints(SetFunctionBreakpointsResponse),
    /// Response to `setInstructionBreakpoints` request
    ///
    /// Specification: [SetInstructionBreakpoints request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetInstructionBreakpoints)
    SetInstructionBreakpoints(SetInstructionBreakpointsResponse),
    /// Response to `setVariable` request.
    ///
    /// Specification: [SetVariable request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetVariable)
    SetVariable(SetVariableResponse),
    /// Response to `source` request.
    ///
    /// Specification: [Sources request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Source)
    Source(SourceResponse),
    /// Response to stackTrace request.
    ///
    /// Specification: [StackTrace request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StackTrace)
    StackTrace(StackTraceResponse),
    /// Response to `stepBack` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [StepBack request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepBack)
    StepBack,
    /// Response to `stepIn` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [StepIn request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepIn)
    StepIn,
    /// Response to `stepOut` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [StepOut request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepOut)
    StepOut,
    /// Response to `terminate` request. This is just an acknowledgement, so no body field is required.
    ///
    /// Specification: [Terminate request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Terminate)
    Terminate,
    /// Response to `terminateThreads` request. This is just an acknowledgement, so no body field is
    /// required.
    ///
    /// Specification: [TerminateThreads request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_TerminateThreads)
    TerminateThreads,
    /// Response to threads request.
    ///
    /// Specification: [Threads request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Threads)
    Threads(ThreadsResponse),
    /// Response to `variables` request.
    ///
    /// Specification: [Variables request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Variables)
    Variables(VariablesResponse),
    /// Response to `writeMemory` request.
    ///
    /// Specification: [WriteMemory request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_WriteMemory)
    WriteMemory(Option<WriteMemoryResponse>),
}

/// Represents response to the client.
///
/// Note that unlike the specification, this implementation does not define a ProtocolMessage base
/// interface. Instead, the only common part (the sequence number) is repeated in the struct.
///
/// The command field (which is a string) is used as a tag in the ResponseBody enum, so users
/// of this crate will control it by selecting the appropriate enum variant for the body.
///
/// There is also no separate `ErrorResponse` struct. Instead, `Error` is just a variant of the
/// ResponseBody enum.
///
/// Specification: [Response](https://microsoft.github.io/debug-adapter-protocol/specification#Base_Protocol_Response)
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// Sequence number of the corresponding request.
    #[serde(rename = "request_seq")]
    pub request_seq: i64,
    /// Outcome of the request.
    /// If true, the request was successful and the `body` attribute may contain
    /// the result of the request.
    /// If the value is false, the attribute `message` contains the error in short
    /// form and the `body` may contain additional information (see
    /// `ErrorResponse.body.error`).
    pub success: bool,
    /// Contains the raw error in short form if `success` is false.
    /// This raw error might be interpreted by the client and is not shown in the
    /// UI.
    /// Some predefined values exist.
    /// Values:
    /// 'cancelled': request was cancelled.
    /// etc.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub message: Option<ResponseMessage>,
    /// Contains request result if success is true and error details if success is
    /// false.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub body: Option<ResponseBody>,
}

impl Response {
    /// Create a successful response for a given request. The sequence number will be copied
    /// from `request`, `message` will be `None` (as its neither cancelled nor an error).
    /// The `body` argument contains the response itself.
    pub fn make_success(request: &Request, body: ResponseBody) -> Self {
        Self {
            request_seq: request.seq,
            success: true,
            message: None,
            body: Some(body), // to love
        }
    }

    /// Create an error response for a given request. The sequence number will be copied
    /// from the request, `message` will be `None` (as its neither cancelled nor an error).
    ///
    /// ## Arguments
    ///
    ///   * `req`: The request this response corresponds to.
    ///   * `body`: The body of the response to attach.
    pub fn make_error(req: &Request, error: &str) -> Self {
        Self {
            request_seq: req.seq,
            success: false,
            message: Some(ResponseMessage::Error(error.to_string())),
            body: None,
        }
    }

    /// Create a cancellation response for the given request. The sequence number will be copied
    /// from the request, message will be [`ResponseMessage::Cancelled`], `success` will be false,
    /// and `body` will be `None`.
    pub fn make_cancel(req: &Request) -> Self {
        Self {
            request_seq: req.seq,
            success: false,
            message: Some(ResponseMessage::Cancelled),
            body: None,
        }
    }

    /// Create an acknowledgement response. This is a shorthand for responding to requests
    /// where the response does not require a body.
    pub fn make_ack(req: &Request) -> Result<Self, ServerError> {
        match req.command {
            Command::Attach(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Attach),
            }),
            Command::ConfigurationDone => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::ConfigurationDone),
            }),
            Command::Disconnect(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Disconnect),
            }),
            Command::Goto(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Goto),
            }),
            Command::Launch(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Launch),
            }),
            Command::Next(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Next),
            }),
            Command::Pause(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Pause),
            }),
            Command::Restart(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Next),
            }),
            Command::RestartFrame(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::RestartFrame),
            }),
            Command::ReverseContinue(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::ReverseContinue),
            }),
            Command::StepBack(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::StepBack),
            }),
            Command::StepIn(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::StepIn),
            }),
            Command::StepOut(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::StepOut),
            }),
            Command::Terminate(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::Terminate),
            }),
            Command::TerminateThreads(_) => Ok(Self {
                request_seq: req.seq,
                success: true,
                message: None,
                body: Some(ResponseBody::TerminateThreads),
            }),
            _ => Err(ServerError::ProtocolError {
                reason: "can't prepare ack unknown command".to_string(),
            }),
        }
    }

    pub fn empty() -> Self {
        Self {
            request_seq: 0,
            success: false,
            message: None,
            body: Some(ResponseBody::Empty),
        }
    }
}
