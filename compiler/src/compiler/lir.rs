use super::error::CompilerError;
use crate::{builtin_functions::BuiltinFunction, hir, input::Input};
use itertools::Itertools;
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lir {
    pub instructions: Vec<Instruction>,
}

pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Instruction {
    /// Pushes an int.
    CreateInt(u64),

    /// Pushes a text.
    CreateText(String),

    /// Pushes a symbol.
    CreateSymbol(String),

    /// Pops 2 * num_entries items, pushes a struct.
    ///
    /// a, key, value, key, value, ..., key, value -> a, pointer to struct
    CreateStruct {
        num_entries: usize,
    },

    /// Pushes a closure.
    ///
    /// a -> a, pointer to closure
    CreateClosure {
        captured: Vec<StackOffset>,
        num_args: usize,
        body: Vec<Instruction>,
    },

    /// Pushes a builtin function.
    ///
    /// a -> a, builtin
    CreateBuiltin(BuiltinFunction),

    /// Leaves the top stack item untouched, but removes n below.
    PopMultipleBelowTop(usize),

    /// Pushes an item from back in the stack on the stack again.
    PushFromStack(StackOffset),

    /// Pops a closure and num_args arguments, pushes the current instruction
    /// pointer, all captured variables, and arguments, and then changes the
    /// instruction pointer to the first instruction of the closure.
    ///
    /// a, arg1, arg2, ..., argN, closure -> a, caller, captured vars, arg1, arg2, ..., argN
    ///
    /// Later, when the closure returns (perhaps many instructions after this
    /// one), the stack will contain the result:
    ///
    /// a, arg1, arg2, ..., argN, closure -> a, return value from closure
    Call {
        num_args: usize,
    },

    /// Pops a boolean condition and a reason. If the condition is true, it
    /// just pushes Nothing. If the condition is false, it panic with the
    /// reason.
    ///
    /// a, condition, reason -> a, Nothing
    Needs,

    /// Returns from the current closure to the original caller.
    ///
    /// a, caller, return value -> a, return value
    Return,

    /// Indicates that a fuzzable closure sits at the top of the stack.
    RegisterFuzzableClosure(hir::Id),

    TraceValueEvaluated(hir::Id),
    TraceCallStarts {
        id: hir::Id,
        num_args: usize,
    },
    TraceCallEnds,
    TraceNeedsStarts {
        id: hir::Id,
    },
    TraceNeedsEnds,
    TraceModuleStarts {
        input: Input,
    },
    TraceModuleEnds,

    Error {
        id: hir::Id,
        error: CompilerError,
    },
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::CreateInt(int) => write!(f, "createInt {int}"),
            Instruction::CreateText(text) => write!(f, "createText {text:?}"),
            Instruction::CreateSymbol(symbol) => write!(f, "createSymbol {symbol}"),
            Instruction::CreateStruct { num_entries } => {
                write!(f, "createStruct {num_entries}")
            }
            Instruction::CreateClosure {
                captured,
                num_args,
                body: instructions,
            } => {
                write!(
                    f,
                    "createClosure with {num_args} {} capturing {}",
                    if *num_args == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                    if captured.is_empty() {
                        "nothing".to_string()
                    } else {
                        captured.iter().join(", ")
                    }
                )?;
                for instruction in instructions {
                    let indented = format!("{instruction}")
                        .lines()
                        .map(|line| format!("  {line}"))
                        .join("\n");
                    write!(f, "\n{indented}")?;
                }
                Ok(())
            }
            Instruction::CreateBuiltin(builtin_function) => {
                write!(f, "createBuiltin {builtin_function:?}")
            }
            Instruction::PopMultipleBelowTop(count) => {
                write!(f, "popMultipleBelowTop {count}")
            }
            Instruction::PushFromStack(offset) => write!(f, "pushFromStack {offset}"),
            Instruction::Call { num_args } => {
                write!(f, "call with {num_args} arguments")
            }
            Instruction::Needs => write!(f, "needs"),
            Instruction::Return => write!(f, "return"),
            Instruction::RegisterFuzzableClosure(hir_id) => {
                write!(f, "registerFuzzableClosure {hir_id}")
            }
            Instruction::TraceValueEvaluated(hir_id) => {
                write!(f, "traceValueEvaluated {hir_id}")
            }
            Instruction::TraceCallStarts { id, num_args } => {
                write!(f, "traceCallStarts {id} ({num_args} args)")
            }
            Instruction::TraceCallEnds => write!(f, "traceCallEnds"),
            Instruction::TraceNeedsStarts { id } => {
                write!(f, "traceNeedsStarts {id}")
            }
            Instruction::TraceNeedsEnds => write!(f, "traceNeedsEnds"),
            Instruction::TraceModuleStarts { input } => write!(f, "traceModuleStarts {input}"),
            Instruction::TraceModuleEnds => write!(f, "traceModuleEnds"),
            Instruction::Error { id, error } => write!(f, "error at {id}: {error:?}"),
        }
    }
}
impl Display for Lir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for instruction in &self.instructions {
            writeln!(f, "{instruction}")?;
        }
        Ok(())
    }
}
