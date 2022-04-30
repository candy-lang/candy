use crate::{builtin_functions::BuiltinFunction, hir};
use std::{collections::HashMap, fmt::Display};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lir {
    pub chunks: Vec<Chunk>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub num_args: usize,
    pub instructions: Vec<Instruction>,
}
pub type ChunkIndex = usize;
pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Pushes a closure that captures the whole stack.
    ///
    /// a -> a, pointer to closure
    CreateClosure(ChunkIndex),

    /// Leaves the top stack item untouched, but removes n below.
    PopMultipleBelowTop(usize),

    /// Pushes an item from back in the stack on the stack again.
    PushFromStack(StackOffset),

    /// Pops a closure and the number of arguments it requires, pushes the
    /// current instruction pointer, all captured variables, and arguments, and
    /// then executes the closure.
    ///
    /// a, arg1, arg2, ..., argN, closure -> a, caller, captured vars, arg1, arg2, ..., argN
    ///
    /// When the closure returns, the stack will contain the result:
    ///
    /// a, arg1, arg2, ..., argN, closure -> a, return value from closure
    Call,

    /// Returns from the current closure to the original caller.
    ///
    /// a, caller, return value -> a, return value
    Return,

    /// Builtins. They all behave differently.
    Builtin(BuiltinFunction),

    /// Indicates that a value for the given id was evaluated and is at the top
    /// of the stack.
    DebugValueEvaluated(hir::Id),

    Error(hir::Id),
}

impl Display for Lir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, chunk) in self.chunks.iter().enumerate() {
            writeln!(f, "Chunk {} ({} args)", i, chunk.num_args)?;
            for instruction in &chunk.instructions {
                write!(f, "  ")?; // indent actual instructions
                match instruction {
                    Instruction::CreateInt(int) => writeln!(f, "createInt {}", int),
                    Instruction::CreateText(text) => writeln!(f, "createText {:?}", text),
                    Instruction::CreateSymbol(symbol) => writeln!(f, "createSymbol {}", symbol),
                    Instruction::CreateStruct { num_entries } => {
                        writeln!(f, "createStruct {}", num_entries)
                    }
                    Instruction::CreateClosure(chunk) => {
                        writeln!(f, "createClosure, chunk {}", chunk)
                    }
                    Instruction::PopMultipleBelowTop(count) => {
                        writeln!(f, "popMultipleBelowTop {}", count)
                    }
                    Instruction::PushFromStack(offset) => writeln!(f, "pushFromStack {}", offset),
                    Instruction::Call => writeln!(f, "call"),
                    Instruction::Return => writeln!(f, "return"),
                    Instruction::Builtin(builtin_function) => {
                        writeln!(f, "builtin {:?}", builtin_function)
                    }
                    Instruction::DebugValueEvaluated(hir_id) => {
                        writeln!(f, "debugValueEvaluated {}", hir_id)
                    }
                    Instruction::Error(hir_id) => writeln!(f, "error {}", hir_id),
                }?;
            }
        }
        Ok(())
    }
}
