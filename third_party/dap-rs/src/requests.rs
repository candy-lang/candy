use std::{convert::Infallible, num::NonZeroUsize, str::FromStr};

use serde::{de, Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    fromstr_deser,
    types::{
        DataBreakpoint, EvaluateArgumentsContext, ExceptionFilterOptions, ExceptionOptions,
        FunctionBreakpoint, InstructionBreakpoint, Source, SourceBreakpoint, StackFrameFormat,
        SteppingGranularity, ValueFormat, VariablesArgumentsFilter,
    },
};

#[derive(Debug, Clone)]
pub enum PathFormat {
    Path,
    Uri,
    Other(String),
}

impl FromStr for PathFormat {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "path" => Ok(PathFormat::Path),
            "uri" => Ok(PathFormat::Uri),
            other => Ok(PathFormat::Other(other.to_string())),
        }
    }
}

fromstr_deser! {PathFormat}

/// Arguments for an Initialize request.
/// In specification: [Initialize](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Initialize)
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    /// The ID of the client using this adapter.
    #[serde(rename = "clientID")]
    pub client_id: Option<String>,
    /// The human-readable name of the client using this adapter.
    pub client_name: Option<String>,
    /// The ID of the debug adapter.
    #[serde(rename = "adapterID")]
    pub adapter_id: String,
    /// The ISO-639 locale of the client using this adapter, e.g. en-US or de-CH.
    pub locale: Option<String>,
    /// If true all line numbers are 1-based (default).
    pub lines_start_at1: Option<bool>,
    /// If true all column numbers are 1-based (default).
    pub columns_start_at1: Option<bool>,
    /// Determines in what format paths are specified. The default is `path`, which
    /// is the native format.
    pub path_format: Option<PathFormat>,
    /// Client supports the `type` attribute for variables.
    pub supports_variable_type: Option<bool>,
    /// Client supports the paging of variables.
    pub supports_variable_paging: Option<bool>,
    /// Client supports the `runInTerminal` request.
    pub supports_run_in_terminal_request: Option<bool>,
    /// Client supports memory references.
    pub supports_memory_references: Option<bool>,
    /// Client supports progress reporting.
    pub supports_progress_reporting: Option<bool>,
    /// Client supports the `invalidated` event.
    pub supports_invalidated_event: Option<bool>,
    /// Client supports the `memory` event.
    pub supports_memory_event: Option<bool>,
    /// Client supports the `argsCanBeInterpretedByShell` attribute on the `runInTerminal` request.
    pub supports_args_can_be_interpreted_by_shell: Option<bool>,
}

/// Arguments for an SetBreakpoints request.
/// In specification: [SetBreakpoints](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Initialize)
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsArguments {
    /// The source location of the breakpoints, either `source.path` or
    /// `source.sourceReference` must be specified.
    pub source: Source,
    /// The code locations of the breakpoints.
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    /// Deprecated: The code locations of the breakpoints.
    #[deprecated]
    pub lines: Option<Vec<usize>>,
    /// A value of true indicates that the underlying source has been modified
    /// which results in new breakpoint locations.
    pub source_modified: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "camelCase"))]
pub struct CancelArguments {
    /// The ID (attribute `seq`) of the request to cancel. If missing no request is
    /// cancelled.
    /// Both a `requestId` and a `progressId` can be specified in one request.
    pub request_id: Option<usize>,
    /// The ID (attribute `progressId`) of the progress to cancel. If missing no
    /// progress is cancelled.
    /// Both a `requestId` and a `progressId` can be specified in one request.
    pub progress_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetExceptionBreakpointsArguments {
    /// Set of exception filters specified by their ID. The set of all possible
    /// exception filters is defined by the `exceptionBreakpointFilters`
    /// capability. The `filter` and `filterOptions` sets are additive.
    pub filters: Vec<String>,
    /// Set of exception filters and their options. The set of all possible
    /// exception filters is defined by the `exceptionBreakpointFilters`
    /// capability. This attribute is only honored by a debug adapter if the
    /// corresponding capability `supportsExceptionFilterOptions` is true. The
    /// `filter` and `filterOptions` sets are additive.
    pub filter_options: Option<Vec<ExceptionFilterOptions>>,
    /// Configuration options for selected exceptions.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportsExceptionOptions` is true.
    pub exception_options: Option<Vec<ExceptionOptions>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetFunctionBreakpointsArguments {
    /// The function names of the breakpoints.
    pub breakpoints: Vec<FunctionBreakpoint>,
}

/// Arguments for a Launch request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct LaunchRequestArguments {
    /// If true, the launch request should launch the program without enabling
    /// debugging.
    pub no_debug: Option<bool>,
    /// Arbitrary data from the previous, restarted session.
    /// The data is sent as the `restart` attribute of the `terminated` event.
    /// The client should leave the data intact.
    ///
    /// Rust-specific: this data must be a string. Server requiring storing binary data should use
    /// an encoding that is suitable for string (e.g. base85 or similar).
    #[serde(rename = "__restart")]
    pub restart_data: Option<Value>,

    /// the program to execute
    /// (extension to the specification, see: codelldb)
    pub program: Option<String>,

    /// process arguments
    /// (extension to the specification, see: lldb-vscode)
    pub args: Option<Vec<String>>,

    /// process working directory
    /// (extension to the specification, see: lldb-vscode)
    pub cwd: Option<String>,

    /// debuggee environment. The expected format is
    /// array of strings, each string has the format of NAME=VALUE
    /// (extension to the specification, see: lldb-vscode)
    pub env: Option<Vec<String>>,
}

/// Arguments for an Attach request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct AttachRequestArguments {
    /// Arbitrary data from the previous, restarted session.
    /// The data is sent as the `restart` attribute of the `terminated` event.
    /// The client should leave the data intact.
    #[serde(rename = "__restart")]
    pub restart_data: Option<Value>,
}

/// Arguments for a BreakpointLocations request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct BreakpointLocationsArguments {
    /// The source location of the breakpoints, either `source.path` or
    /// `source.reference` must be specified.
    pub source: Source,
    /// Start line of range to search possible breakpoint locations in. If only the
    /// line is specified, the request returns all possible locations in that line.
    pub line: usize,
    /// Start position within `line` to search possible breakpoint locations in. It
    /// is measured in UTF-16 code units and the client capability
    /// `columnsStartAt1` determines whether it is 0- or 1-based. If no column is
    /// given, the first position in the start line is assumed.
    pub column: Option<usize>,
    /// End line of range to search possible breakpoint locations in. If no end
    /// line is given, then the end line is assumed to be the start line.
    pub end_line: Option<usize>,
    /// End position within `endLine` to search possible breakpoint locations in.
    /// It is measured in UTF-16 code units and the client capability
    /// `columnsStartAt1` determines whether it is 0- or 1-based. If no end column
    /// is given, the last position in the end line is assumed.
    pub end_column: Option<usize>,
}

/// Arguments for a Completions request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct CompletionsArguments {
    /// Returns completions in the scope of this stack frame. If not specified, the
    /// completions are returned for the global scope.
    pub frame_id: Option<usize>,
    /// One or more source lines. Typically this is the text users have typed into
    /// the debug console before they asked for completion.
    pub text: String,
    /// The position within `text` for which to determine the completion proposals.
    /// It is measured in UTF-16 code units and the client capability
    /// `columnsStartAt1` determines whether it is 0- or 1-based.
    pub column: usize,
    /// A line for which to determine the completion proposals. If missing the
    /// first line of the text is assumed.
    pub line: Option<usize>,
}

/// Arguments for a Continue request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ContinueArguments {
    /// Specifies the active thread. If the debug adapter supports single thread
    /// execution (see `supportsSingleThreadExecutionRequests`) and the argument
    /// `singleThread` is true, only the thread with this ID is resumed.
    pub thread_id: usize,
    /// If this flag is true, execution is resumed only for the thread with given
    /// `threadId`.
    pub single_thread: Option<bool>,
}

/// Arguments for a DataBreakpointInfo request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct DataBreakpointInfoArguments {
    /// Reference to the variable container if the data breakpoint is requested for
    /// a child of the container.
    pub variables_reference: Option<usize>,
    /// The name of the variable's child to obtain data breakpoint information for.
    /// If `variablesReference` isn't specified, this can be an expression.
    pub name: String,
}

/// Arguments for a Disassamble request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct DisassembleArguments {
    /// Memory reference to the base location containing the instructions to
    /// disassemble.
    pub memory_reference: String,
    /// Offset (in bytes) to be applied to the reference location before
    /// disassembling. Can be negative.
    pub offset: Option<usize>,
    /// Offset (in instructions) to be applied after the byte offset (if any)
    /// before disassembling. Can be negative.
    pub instruction_offset: Option<usize>,
    /// Number of instructions to disassemble starting at the specified location
    /// and offset.
    /// An adapter must return exactly this usize of instructions - any
    /// unavailable instructions should be replaced with an implementation-defined
    /// 'invalid instruction' value.
    pub instruction_count: usize,
    /// If true, the adapter should attempt to resolve memory addresses and other
    /// values to symbolic names.
    pub resolve_symbols: Option<bool>,
}

/// Arguments for a Disconnect request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct DisconnectArguments {
    /// A value of true indicates that this `disconnect` request is part of a
    /// restart sequence.
    pub restart: Option<bool>,
    /// Indicates whether the debuggee should be terminated when the debugger is
    /// disconnected.
    /// If unspecified, the debug adapter is free to do whatever it thinks is best.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportTerminateDebuggee` is true.
    pub terminate_debuggee: Option<bool>,
    /// Indicates whether the debuggee should stay suspended when the debugger is
    /// disconnected.
    /// If unspecified, the debuggee should resume execution.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportSuspendDebuggee` is true.
    pub suspend_debuggee: Option<bool>,
}

/// Arguments for a Evaluate request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct EvaluateArguments {
    /// The expression to evaluate.
    pub expression: String,
    /// Evaluate the expression in the scope of this stack frame. If not specified,
    /// the expression is evaluated in the global scope.
    pub frame_id: Option<usize>,
    /// The context in which the evaluate request is used.
    /// Values:
    /// 'variables': evaluate is called from a variables view context.
    /// 'watch': evaluate is called from a watch view context.
    /// 'repl': evaluate is called from a REPL context.
    /// 'hover': evaluate is called to generate the debug hover contents.
    /// This value should only be used if the corresponding capability
    /// `supportsEvaluateForHovers` is true.
    /// 'clipboard': evaluate is called to generate clipboard contents.
    /// This value should only be used if the corresponding capability
    /// `supportsClipboardContext` is true.
    /// etc.
    pub context: Option<EvaluateArgumentsContext>,
    /// Specifies details on how to format the result.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportsValueFormattingOptions` is true.
    pub format: Option<ValueFormat>,
}

/// Arguments for a ExceptionInfo request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ExceptionInfoArguments {
    /// Thread for which exception information should be retrieved.
    pub thread_id: usize,
}

/// Arguments for a Goto request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct GotoArguments {
    /// Set the goto target for this thread.
    pub thread_id: usize,
    /// The location where the debuggee will continue to run.
    pub target_id: usize,
}

/// Arguments for a GotoTargets request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct GotoTargetsArguments {
    /// The source location for which the goto targets are determined.
    pub source: Source,
    /// The line location for which the goto targets are determined.
    pub line: usize,
    /// The position within `line` for which the goto targets are determined. It is
    /// measured in UTF-16 code units and the client capability `columnsStartAt1`
    /// determines whether it is 0- or 1-based.
    pub column: Option<usize>,
}

/// Arguments for a Modules request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ModulesArguments {
    /// The index of the first module to return, if omitted modules start at 0.
    pub start_module: Option<usize>,
    /// The usize of modules to return. If `moduleCount` is not specified or 0,
    /// all modules are returned.
    pub module_count: Option<usize>,
}

/// Arguments for a Next request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct NextArguments {
    /// Specifies the thread for which to resume execution for one step (of the
    /// given granularity).
    pub thread_id: usize,
    /// If this flag is true, all other suspended threads are not resumed.
    pub single_thread: Option<bool>,
    /// Stepping granularity. If no granularity is specified, a granularity of
    /// `statement` is assumed.
    pub granularity: Option<SteppingGranularity>,
}

/// Arguments for a Pause request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct PauseArguments {
    /// Pause execution for this thread.
    pub thread_id: usize,
}

/// Arguments for a ReadMemory request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ReadMemoryArguments {
    /// Memory reference to the base location from which data should be read.
    pub memory_reference: String,
    /// Offset (in bytes) to be applied to the reference location before reading
    /// data. Can be negative.
    pub offset: Option<usize>,
    /// Number of bytes to read at the specified location and offset.
    pub count: usize,
}

/// Arguments for a ReadMemory request.
#[derive(Deserialize, Debug, Clone)]
pub enum RestartArguments {
    AttachArguments(AttachRequestArguments),
    LaunchArguments(LaunchRequestArguments),
}

/// Arguments for a RestartFrame request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct RestartFrameArguments {
    /// Restart this stackframe.
    pub frame_id: usize,
}

/// Arguments for a ReverseContinue request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ReverseContinueArguments {
    /// Specifies the active thread. If the debug adapter supports single thread
    /// execution (see `supportsSingleThreadExecutionRequests`) and the
    /// `singleThread` argument is true, only the thread with this ID is resumed.
    pub thread_id: usize,
    /// If this flag is true, backward execution is resumed only for the thread
    /// with given `threadId`.
    pub single_thread: Option<bool>,
}

/// Arguments for a Scopes request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct ScopesArguments {
    /// Retrieve the scopes for this stackframe.
    pub frame_id: usize,
}

/// Arguments for a SetDataBreakpoints request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetDataBreakpointsArguments {
    /// The contents of this array replaces all existing data breakpoints. An empty
    /// array clears all data breakpoints.
    pub breakpoints: Vec<DataBreakpoint>,
}

/// Arguments for a SetExpression request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetExpressionArguments {
    /// The l-value expression to assign to.
    pub expression: String,
    /// The value expression to assign to the l-value expression.
    pub value: String,
    /// Evaluate the expressions in the scope of this stack frame. If not
    /// specified, the expressions are evaluated in the global scope.
    pub frame_id: Option<usize>,
    /// Specifies how the resulting value should be formatted.
    pub format: Option<ValueFormat>,
}

/// Arguments for a SetExpression request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetInstructionBreakpointsArguments {
    /// The instruction references of the breakpoints
    pub breakpoints: Vec<InstructionBreakpoint>,
}

/// Arguments for a SetVariable request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SetVariableArguments {
    /// The reference of the variable container.
    pub variables_reference: usize,
    /// The name of the variable in the container.
    pub name: String,
    /// The value of the variable.
    pub value: String,
    /// Specifies details on how to format the response value.
    pub format: Option<ValueFormat>,
}

/// Arguments for a Source request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct SourceArguments {
    /// Specifies the source content to load. Either `source.path` or
    /// `source.sourceReference` must be specified.
    pub source: Option<Source>,
    /// The reference to the source. This is the same as `source.sourceReference`.
    /// This is provided for backward compatibility since old clients do not
    /// understand the `source` attribute.
    pub source_reference: usize,
}

/// Arguments for a StackTrace request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct StackTraceArguments {
    /// Retrieve the stacktrace for this thread.
    pub thread_id: usize,
    /// The index of the first frame to return, if omitted frames start at 0.
    pub start_frame: Option<usize>,
    /// The maximum usize of frames to return. If levels is not specified or 0,
    /// all frames are returned.
    pub levels: Option<usize>,
    /// Specifies details on how to format the stack frames.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportsValueFormattingOptions` is true.
    pub format: Option<StackFrameFormat>,
}

/// Arguments for a StepBack request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct StepBackArguments {
    /// Specifies the thread for which to resume execution for one step backwards
    /// (of the given granularity).
    pub thread_id: usize,
    /// If this flag is true, all other suspended threads are not resumed.
    pub single_thread: Option<bool>,
    /// Stepping granularity to step. If no granularity is specified, a granularity
    /// of `statement` is assumed.
    pub granularity: Option<SteppingGranularity>,
}

/// Arguments for a StepIn request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct StepInArguments {
    /// Specifies the thread for which to resume execution for one step-into (of
    /// the given granularity).
    pub thread_id: usize,
    /// If this flag is true, all other suspended threads are not resumed.
    pub single_thread: Option<bool>,
    /// Id of the target to step into.
    pub target_id: Option<usize>,
    /// Stepping granularity. If no granularity is specified, a granularity of
    /// `statement` is assumed.
    pub granularity: Option<SteppingGranularity>,
}

/// Arguments for a StepInTargets request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct StepInTargetsArguments {
    /// The stack frame for which to retrieve the possible step-in targets.
    pub frame_id: usize,
}

/// Arguments for a StepOut request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct StepOutArguments {
    /// Specifies the thread for which to resume execution for one step-out (of the
    /// given granularity).
    pub thread_id: usize,
    /// If this flag is true, all other suspended threads are not resumed.
    pub single_thread: Option<bool>,
    /// Stepping granularity. If no granularity is specified, a granularity of
    /// `statement` is assumed.
    pub granularity: Option<SteppingGranularity>,
}

/// Arguments for a Terminate request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct TerminateArguments {
    /// A value of true indicates that this `terminate` request is part of a
    /// restart sequence.
    pub restart: Option<bool>,
}

/// Arguments for a TerminateThreads request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct TerminateThreadsArguments {
    /// Ids of threads to be terminated.
    pub thread_ids: Option<Vec<usize>>,
}

/// Arguments for a Variables request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct VariablesArguments {
    /// The Variable reference.
    pub variables_reference: usize,
    /// Filter to limit the child variables to either named or indexed. If omitted,
    /// both types are fetched.
    /// Values: 'indexed', 'named'
    pub filter: Option<VariablesArgumentsFilter>,
    /// The index of the first variable to return, if omitted children start at 0.
    pub start: Option<usize>,
    /// The usize of variables to return. If count is missing or 0, all variables
    /// are returned.
    pub count: Option<usize>,
    /// Specifies details on how to format the Variable values.
    /// The attribute is only honored by a debug adapter if the corresponding
    /// capability `supportsValueFormattingOptions` is true.
    pub format: Option<ValueFormat>,
}

/// Arguments for a WriteMemory request.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub struct WriteMemoryArguments {
    /// Memory reference to the base location to which data should be written.
    pub memory_reference: String,
    /// Offset (in bytes) to be applied to the reference location before writing
    /// data. Can be negative.
    pub offset: Option<usize>,
    /// Property to control partial writes. If true, the debug adapter should
    /// attempt to write memory even if the entire memory region is not writable.
    /// In such a case the debug adapter should stop after hitting the first byte
    /// of memory that cannot be written and return the usize of bytes written in
    /// the response via the `offset` and `bytesWritten` properties.
    /// If false or missing, a debug adapter should attempt to verify the region is
    /// writable before writing, and fail the response if it is not.
    pub allow_partial: Option<bool>,
    /// Bytes to write, encoded using base64.
    pub data: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(
    tag = "command",
    content = "arguments",
    rename_all(deserialize = "camelCase", serialize = "snake_case")
)]
pub enum Command {
    /// The attach request is sent from the client to the debug adapter to attach to a debuggee that
    /// is already running.
    /// kSince attaching is debugger/runtime specific, the arguments for this request are not part of
    /// this specification.
    ///
    /// Specification: [Attach request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Attach)
    Attach(AttachRequestArguments),
    /// The `breakpointLocations` request returns all possible locations for source breakpoints in a
    /// given range.
    /// Clients should only call this request if the corresponding capability
    /// `supportsBreakpointLocationsRequest` is true.
    ///
    /// Specification: [BreakpointLocations request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_BreakpointLocations)
    BreakpointLocations(BreakpointLocationsArguments),
    /// Returns a list of possible completions for a given caret position and text.
    /// Clients should only call this request if the corresponding capability
    /// `supportsCompletionsRequest` is true.
    ///
    /// Specification: [Completions request]: https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Completions
    Completions(CompletionsArguments),
    /// This request indicates that the client has finished initialization of the debug adapter.
    /// So it is the last request in the sequence of configuration requests (which was started by the initialized event).
    /// Clients should only call this request if the corresponding capability supportsConfigurationDoneRequest is true.
    ///
    /// Specification: [ConfigurationDone](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ConfigurationDone)
    ConfigurationDone,
    /// The request resumes execution of all threads. If the debug adapter supports single thread
    /// execution (see capability `supportsSingleThreadExecutionRequests`), setting the singleThread
    /// argument to true resumes only the specified thread. If not all threads were resumed, the
    /// `allThreadsContinued` attribute of the response should be set to false.
    ///
    /// Specification: [Continue request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Continue)
    Continue(ContinueArguments),
    /// Obtains information on a possible data breakpoint that could be set on an expression or
    /// variable.
    /// Clients should only call this request if the corresponding capability supportsDataBreakpoints
    /// is true.
    ///
    /// Specification: [DataBreakpointInfo request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_DataBreakpointInfo)
    DataBreakpointInfo(DataBreakpointInfoArguments),
    /// Disassembles code stored at the provided location.
    /// Clients should only call this request if the corresponding capability
    /// `supportsDisassembleRequest` is true.
    ///
    /// Specification: [Disassamble request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Disassemble)
    Disassamble(DisassembleArguments),
    /// The `disconnect` request asks the debug adapter to disconnect from the debuggee (thus ending
    /// the debug session) and then to shut down itself (the debug adapter).
    /// In addition, the debug adapter must terminate the debuggee if it was started with the `launch`
    /// request. If an `attach` request was used to connect to the debuggee, then the debug adapter
    /// must not terminate the debuggee.
    /// This implicit behavior of when to terminate the debuggee can be overridden with the
    /// `terminateDebuggee` argument (which is only supported by a debug adapter if the corresponding
    /// capability `supportTerminateDebuggee` is true).
    ///
    /// Specification: [Disconnect request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Disconnect)
    Disconnect(DisconnectArguments),
    /// Evaluates the given expression in the context of the topmost stack frame.
    /// The expression has access to any variables and arguments that are in scope.
    ///
    /// Specification: [Evaluate arguments](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Evaluate)
    Evaluate(EvaluateArguments),
    /// Retrieves the details of the exception that caused this event to be raised.
    /// Clients should only call this request if the corresponding capability
    /// `supportsExceptionInfoRequest` is true.
    ///
    /// Specification: [ExceptionInfo request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ExceptionInfo)
    ExceptionInfo(ExceptionInfoArguments),
    /// The request sets the location where the debuggee will continue to run.
    /// This makes it possible to skip the execution of code or to execute code again.
    /// The code between the current location and the goto target is not executed but skipped.
    /// The debug adapter first sends the response and then a stopped event with reason goto.
    /// Clients should only call this request if the corresponding capability
    /// `supportsGotoTargetsRequest` is true (because only then goto targets exist that can be passed
    /// as arguments).
    ///
    /// Specification: [Goto request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Goto)
    Goto(GotoArguments),
    /// This request retrieves the possible goto targets for the specified source location.
    /// These targets can be used in the goto request.
    /// Clients should only call this request if the corresponding capability
    /// `supportsGotoTargetsRequest` is true.
    ///
    /// Specification: [GotoTargets request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_GotoTargets)
    GotoTargets(GotoTargetsArguments),
    /// The initialize request is sent as the first request from the client to the debug adapter in
    /// order to configure it with client capabilities and to retrieve capabilities from the debug
    /// adapter.
    ///
    /// Until the debug adapter has responded with an initialize response, the client must not send any
    /// additional requests or events to the debug adapter.
    ///
    /// In addition the debug adapter is not allowed to send any requests or events to the client until
    ///  it has responded with an initialize response.
    ///
    /// The initialize request may only be sent once.
    ///
    /// Specification: [InitializeRequest](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Initialize)
    Initialize(InitializeArguments),
    /// This launch request is sent from the client to the debug adapter to start the debuggee with
    /// or without debugging (if noDebug is true).
    /// Since launching is debugger/runtime specific, the arguments for this request are not part of
    /// this specification.
    ///
    /// Specification: [Launch request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Launch)
    Launch(LaunchRequestArguments),
    /// Retrieves the set of all sources currently loaded by the debugged process.
    /// Clients should only call this request if the corresponding capability supportsLoadedSourcesRequest is true.
    ///
    /// Specification: [LoadedSources request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_LoadedSources)
    LoadedSources,
    /// Modules can be retrieved from the debug adapter with this request which can either return
    /// all modules or a range of modules to support paging.
    /// Clients should only call this request if the corresponding capability
    /// `supportsModulesRequest` is true.
    ///
    /// Specification: [Modules request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Modules)
    Modules(ModulesArguments),
    /// The request executes one step (in the given granularity) for the specified thread and allows
    /// all other threads to run freely by resuming them.
    /// If the debug adapter supports single thread execution (see capability
    /// `supportsSingleThreadExecutionRequests`), setting the `singleThread` argument to true
    /// prevents other suspended threads from resuming.
    /// The debug adapter first sends the response and then a stopped event (with reason step) after
    /// the step has completed.
    ///
    /// Specification: [Next request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Next)
    Next(NextArguments),
    /// The request suspends the debuggee.
    /// The debug adapter first sends the response and then a `stopped` event (with reason pause)
    /// after the thread has been paused successfully.
    ///
    /// Specification: [Pause request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Pause)
    Pause(PauseArguments),
    /// Reads bytes from memory at the provided location.
    /// Clients should only call this request if the corresponding capability
    /// `supportsReadMemoryRequest` is true.
    ///
    /// Specification: [ReadMemory request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ReadMemory)
    ReadMemory(ReadMemoryArguments),
    /// Restarts a debug session. Clients should only call this request if the corresponding
    /// capability supportsRestartRequest is true.
    /// If the capability is missing or has the value false, a typical client emulates restart by
    /// terminating the debug adapter first and then launching it anew.
    ///
    /// Specification: [Restart request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Restart)
    Restart(RestartArguments),
    /// The request restarts execution of the specified stackframe.
    /// The debug adapter first sends the response and then a `stopped` event (with reason `restart`)
    /// after the restart has completed.
    /// Clients should only call this request if the corresponding capability `supportsRestartFrame`
    /// is true.
    ///
    /// Specification: [RestartFrame request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_RestartFrame)
    RestartFrame(RestartFrameArguments),
    /// The request resumes backward execution of all threads. If the debug adapter supports single
    /// thread execution (see capability `supportsSingleThreadExecutionRequests`), setting the
    /// singleThread argument to true resumes only the specified thread. If not all threads were
    /// resumed, the `allThreadsContinued` attribute of the response should be set to false.
    /// Clients should only call this request if the corresponding capability supportsStepBack is true.
    ///
    /// Specification: [ReverseContinue request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_ReverseContinue)
    ReverseContinue(ReverseContinueArguments),
    /// The request returns the variable scopes for a given stackframe ID.
    ///
    /// Specification: [Scopes request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Scopes)
    Scopes(ScopesArguments),
    /// Sets multiple breakpoints for a single source and clears all previous breakpoints in that source.
    ///
    /// To clear all breakpoint for a source, specify an empty array.
    ///
    /// When a breakpoint is hit, a stopped event (with reason breakpoint) is generated.
    ///
    /// Specification: [SetBreakpointsRequest](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetBreakpoints)
    SetBreakpoints(SetBreakpointsArguments),
    /// Replaces all existing data breakpoints with new data breakpoints.
    /// To clear all data breakpoints, specify an empty array.
    /// When a data breakpoint is hit, a `stopped` event (with reason `data breakpoint`) is generated.
    /// Clients should only call this request if the corresponding capability
    /// `supportsDataBreakpoints` is true.
    ///
    /// Specification: [SetDataBreakpoints request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetDataBreakpoints)
    SetDataBreakpoints(SetDataBreakpointsArguments),
    /// The request configures the debugger’s response to thrown exceptions.
    /// If an exception is configured to break, a stopped event is fired (with reason exception).
    /// Clients should only call this request if the corresponding capability exceptionBreakpointFilters returns one or more filters.
    ///
    /// Specification: [SetExceptionBreakpoints](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetExceptionBreakpoints)
    SetExceptionBreakpoints(SetExceptionBreakpointsArguments),
    /// Evaluates the given `value` expression and assigns it to the `expression` which must be a
    /// modifiable l-value.
    /// The expressions have access to any variables and arguments that are in scope of the specified
    /// frame.
    /// Clients should only call this request if the corresponding capability `supportsSetExpression`
    /// is true.
    /// If a debug adapter implements both `setExpression` and `setVariable`, a client uses
    /// `setExpression` if the variable has an `evaluateName` property.
    ///
    /// Specification: [SetExpression request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetExpression)
    SetExpression(SetExpressionArguments),
    /// Replaces all existing function breakpoints with new function breakpoints.
    /// To clear all function breakpoints, specify an empty array.
    /// When a function breakpoint is hit, a stopped event (with reason function breakpoint) is
    /// generated.
    /// Clients should only call this request if the corresponding capability
    /// supportsFunctionBreakpoints is true.
    ///
    /// Specification: [SetFunctionBreakpointsArguments](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetFunctionBreakpoints)
    SetFunctionBreakpoints(SetFunctionBreakpointsArguments),
    /// Replaces all existing instruction breakpoints. Typically, instruction breakpoints would be set from a disassembly window.
    /// To clear all instruction breakpoints, specify an empty array.
    /// When an instruction breakpoint is hit, a `stopped` event (with reason
    /// `instruction breakpoint`) is generated.
    /// Clients should only call this request if the corresponding capability
    /// `supportsInstructionBreakpoints` is true.
    ///
    /// Specification: [SetInstructionBreakpoints request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetInstructionBreakpoints)
    SetInstructionBreakpoints(SetInstructionBreakpointsArguments),
    /// Set the variable with the given name in the variable container to a new value. Clients should
    /// only call this request if the corresponding capability `supportsSetVariable` is true.
    /// If a debug adapter implements both `setVariable` and `setExpression`, a client will only use
    /// `setExpression` if the variable has an `evaluateName` property.
    ///
    /// Specification: [SetVariable request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetVariable)
    SetVariable(SetVariableArguments),
    /// The request retrieves the source code for a given source reference.
    ///
    /// Specification: [Sources request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Source)
    Source(SourceArguments),
    /// The request returns a stacktrace from the current execution state of a given thread.
    /// A client can request all stack frames by omitting the startFrame and levels arguments. For
    /// performance-conscious clients and if the corresponding capability
    /// `supportsDelayedStackTraceLoading` is true, stack frames can be retrieved in a piecemeal way
    /// with the startFrame and levels arguments. The response of the stackTrace request may
    /// contain a totalFrames property that hints at the total number of frames in the stack. If a
    /// client needs this total number upfront, it can issue a request for a single (first) frame
    /// and depending on the value of totalFrames decide how to proceed. In any case a client should
    /// be prepared to receive fewer frames than requested, which is an indication that the end of
    /// the stack has been reached.
    ///
    /// Specification: [StackTrace request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StackTrace)
    StackTrace(StackTraceArguments),
    /// The request executes one backward step (in the given granularity) for the specified thread
    /// and allows all other threads to run backward freely by resuming them.
    /// If the debug adapter supports single thread execution (see capability
    /// `supportsSingleThreadExecutionRequests`), setting the `singleThread` argument to true prevents
    /// other suspended threads from resuming.
    /// The debug adapter first sends the response and then a stopped event (with reason step) after
    /// the step has completed.
    /// Clients should only call this request if the corresponding capability `supportsStepBack` is
    /// true.
    ///
    /// Specification: [StepBack request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepBack)
    StepBack(StepBackArguments),
    /// The request resumes the given thread to step into a function/method and allows all other
    /// threads to run freely by resuming them.
    /// If the debug adapter supports single thread execution (see capability
    /// `supportsSingleThreadExecutionRequests`), setting the `singleThread` argument to true
    /// prevents other suspended threads from resuming.
    ///
    /// Specification: [StepIn request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepIn)
    StepIn(StepInArguments),
    /// This request retrieves the possible step-in targets for the specified stack frame.
    /// These targets can be used in the `stepIn` request.
    /// Clients should only call this request if the corresponding capability
    /// `supportsStepInTargetsRequest` is true.
    ///
    /// Specification: [StepInTargets request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepInTargets)
    StepInTargets(StepInTargetsArguments),
    /// The request resumes the given thread to step out (return) from a function/method and allows
    /// all other threads to run freely by resuming them.
    /// If the debug adapter supports single thread execution (see capability
    /// `supportsSingleThreadExecutionRequests`), setting the singleThread argument to true prevents
    /// other suspended threads from resuming.
    /// The debug adapter first sends the response and then a stopped event (with reason step) after
    /// the step has completed.
    ///
    /// Specification: [StepOut request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepOut)
    StepOut(StepOutArguments),
    /// The terminate request is sent from the client to the debug adapter in order to shut down the
    /// debuggee gracefully. Clients should only call this request if the capability
    /// `supportsTerminateRequest` is true.
    ///
    /// Specification: [Terminate request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Terminate)
    Terminate(TerminateArguments),
    /// The request terminates the threads with the given ids.
    /// Clients should only call this request if the corresponding capability
    /// `supportsTerminateThreadsRequest` is true.
    ///
    /// Specification: [TerminateThreads request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_TerminateThreads)
    TerminateThreads(TerminateThreadsArguments),
    /// The request retrieves a list of all threads.
    ///
    /// Specification: [Threads request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Threads)
    Threads,
    /// Retrieves all child variables for the given variable reference.
    /// A filter can be used to limit the fetched children to either named or indexed children.
    ///
    /// Specification: [Variables request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Variables)
    Variables(VariablesArguments),
    /// Writes bytes to memory at the provided location.
    /// Clients should only call this request if the corresponding capability
    /// `supportsWriteMemoryRequest` is true.
    ///
    /// Specification: [WriteMemory request](https://microsoft.github.io/debug-adapter-protocol/specification#Requests_WriteMemory)
    WriteMemory(WriteMemoryArguments),
    /// The cancel request is used by the client in two situations:
    ///
    /// to indicate that it is no longer interested in the result produced by a specific request
    /// issued earlier to cancel a progress sequence. Clients should only call this request if the
    /// corresponding capability supportsCancelRequest is true.
    ///
    /// Specification: [CancelRequest](https://microsoft.github.io/debug-adapter-protocol/specification#Base_Protocol_Cancel)
    Cancel(CancelArguments),
}

/// Represents a request from a client.
///
/// Note that unlike the specification, this implementation does not define a ProtocolMessage base
/// interface. Instead, the only common part (the sequence number) is repeated in the struct.
///
/// Specification: [Request](https://microsoft.github.io/debug-adapter-protocol/specification#Base_Protocol_Request)
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Request {
    /// Sequence number for the Request.
    ///
    /// From the [specification](https://microsoft.github.io/debug-adapter-protocol/specification#Base_Protocol_ProtocolMessage):
    ///
    /// Sequence number of the message (also known as message ID). The `seq` for
    /// the first message sent by a client or debug adapter is 1, and for each
    /// subsequent message is 1 greater than the previous message sent by that
    /// actor. `seq` can be used to order requests, responses, and events, and to
    /// associate requests with their corresponding responses. For protocol
    /// messages of type `request` the sequence number can be used to cancel the
    /// request.
    pub seq: NonZeroUsize,
    /// The command to execute.
    ///
    /// This is stringly typed in the specification, but represented as an enum for better
    /// ergonomics in Rust code, along with the arguments when present.
    #[serde(flatten)]
    pub command: Command,
}
