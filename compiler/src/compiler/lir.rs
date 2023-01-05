use super::mir::Id;
use crate::{builtin_functions::BuiltinFunction, compiler::hir, module::Module};
use itertools::Itertools;
use num_bigint::BigInt;
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lir {
    pub instructions: Vec<Instruction>,
}

pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Instruction {
    /// Pushes an int.
    CreateInt(BigInt),

    /// Pushes a text.
    CreateText(String),

    /// Pushes a symbol.
    CreateSymbol(String),

    /// Pushes a builtin function.
    ///
    /// a -> a, builtin
    CreateBuiltin(BuiltinFunction),

    /// Pops num_items items, pushes a list.
    ///
    /// a, item, item, ..., item -> a, pointer to list
    CreateList {
        num_items: usize,
    },

    /// Pops 2 * num_fields items, pushes a struct.
    ///
    /// a, key, value, key, value, ..., key, value -> a, pointer to struct
    CreateStruct {
        num_fields: usize,
    },

    /// Pushes a HIR ID.
    CreateHirId(hir::Id),

    /// Pushes a closure.
    ///
    /// a -> a, pointer to closure
    CreateClosure {
        captured: Vec<StackOffset>,
        num_args: usize, // excluding responsible parameter
        body: Vec<Instruction>,
    },

    /// Pushes an item from back in the stack on the stack again.
    PushFromStack(StackOffset),

    /// Leaves the top stack item untouched, but removes n below.
    PopMultipleBelowTop(usize),

    /// Sets up the data stack for a closure execution and then changes the
    /// instruction pointer to the first instruction.
    ///
    /// a, closure, arg1, arg2, ..., argN, responsible -> a, caller, captured vars, arg1, arg2, ..., argN, responsible
    ///
    /// Later, when the closure returns (perhaps many instructions after this
    /// one), the stack will contain the result:
    ///
    /// a, closure, arg1, arg2, ..., argN, responsible ~> a, return value from closure
    Call {
        num_args: usize, // excluding the responsible argument
    },

    /// Like `Call`, but after popping the stack entries for the call itself, it
    /// also pops the given number of local stack entries before actually
    /// executing the call.
    TailCall {
        num_locals_to_pop: usize,
        num_args: usize, // excluding the responsible argument
    },

    /// Returns from the current closure to the original caller. Leaves the data
    /// stack untouched, but pops a caller from the call stack and returns the
    /// instruction pointer to continue where the current function was called.
    Return,

    /// Pops a string path and responsilbe HIR ID and then resolves the path
    /// relative to the current module. Then does different things depending on
    /// whether this is a code or asset module.
    ///
    /// - Code module:
    ///
    ///   Loads and parses the module, then runs the module closure. Later,
    ///   when the module returns, the stack will contain the struct of the
    ///   exported definitions:
    ///
    ///   a, path, responsible ~> a, structOfModuleExports
    ///
    /// - Asset module:
    ///   
    ///   Loads the file and pushes its content onto the stack:
    ///
    ///   a, path, responsible -> a, listOfContentBytes
    UseModule {
        current_module: Module,
    },

    /// Panics. Because the panic instruction only occurs inside the generated
    /// needs function, the reason is already guaranteed to be a text.
    ///
    /// a, reason, responsible -> 💥
    Panic,

    ModuleStarts {
        module: Module,
    },
    ModuleEnds,

    /// a, HIR ID, function, arg1, arg2, ..., argN, responsible -> a
    TraceCallStarts {
        num_args: usize,
    },

    // a, return value -> a
    TraceCallEnds,

    /// a, HIR ID, value -> a
    TraceExpressionEvaluated,

    /// a, HIR ID, closure -> a
    TraceFoundFuzzableClosure,
}

impl Instruction {
    /// Applies the instruction's effect on the stack. After calling it, the
    /// stack will be in the same state as when the control flow continues after
    /// this instruction.
    pub fn apply_to_stack(&self, stack: &mut Vec<Id>, result: Id) {
        match self {
            Instruction::CreateInt(_) => {
                stack.push(result);
            }
            Instruction::CreateText(_) => {
                stack.push(result);
            }
            Instruction::CreateSymbol(_) => {
                stack.push(result);
            }
            Instruction::CreateBuiltin(_) => {
                stack.push(result);
            }
            Instruction::CreateList { num_items } => {
                stack.pop_multiple(*num_items);
                stack.push(result);
            }
            Instruction::CreateStruct { num_fields } => {
                stack.pop_multiple(2 * num_fields); // fields
                stack.push(result);
            }
            Instruction::CreateHirId { .. } => {
                stack.push(result);
            }
            Instruction::CreateClosure { .. } => {
                stack.push(result);
            }
            Instruction::PushFromStack(_) => {
                stack.push(result);
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = stack.pop().unwrap();
                stack.pop_multiple(*n);
                stack.push(top);
            }
            Instruction::Call { num_args } => {
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // closure/builtin
                stack.push(result); // return value
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // closure/builtin
                stack.pop_multiple(*num_locals_to_pop);
                stack.push(result); // return value
            }
            Instruction::Return => {
                // Only modifies the call stack and the instruction pointer.
                // Leaves the return value untouched on the stack.
            }
            Instruction::UseModule { .. } => {
                stack.pop(); // responsible
                stack.pop(); // module path
                stack.push(result); // exported members or bytes of file
            }
            Instruction::Panic => {
                stack.pop(); // responsible
                stack.pop(); // reason
                stack.push(result);
            }
            Instruction::ModuleStarts { .. } => {}
            Instruction::ModuleEnds => {}
            Instruction::TraceCallStarts { num_args } => {
                stack.pop(); // HIR ID
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // callee
            }
            Instruction::TraceCallEnds => {
                stack.pop(); // return value
            }
            Instruction::TraceExpressionEvaluated => {
                stack.pop(); // HIR ID
                stack.pop(); // value
            }
            Instruction::TraceFoundFuzzableClosure => {
                stack.pop(); // HIR ID
                stack.pop(); // value
            }
        }
    }
}

trait StackExt {
    fn pop_multiple(&mut self, n: usize);
}
impl StackExt for Vec<Id> {
    fn pop_multiple(&mut self, n: usize) {
        for _ in 0..n {
            self.pop();
        }
    }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::CreateInt(int) => write!(f, "createInt {int}"),
            Instruction::CreateText(text) => write!(f, "createText {text:?}"),
            Instruction::CreateSymbol(symbol) => write!(f, "createSymbol {symbol}"),
            Instruction::CreateList { num_items } => {
                write!(f, "createList {num_items}")
            }
            Instruction::CreateStruct { num_fields } => {
                write!(f, "createStruct {num_fields}")
            }
            Instruction::CreateHirId(id) => write!(f, "createHirId {id}"),
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
                    },
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
            Instruction::PushFromStack(offset) => write!(f, "pushFromStack {offset}"),
            Instruction::PopMultipleBelowTop(count) => {
                write!(f, "popMultipleBelowTop {count}")
            }
            Instruction::Call { num_args } => {
                write!(f, "call with {num_args} arguments")
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                write!(
                    f,
                    "tail call with {num_locals_to_pop} locals and {num_args} arguments"
                )
            }
            Instruction::Return => write!(f, "return"),
            Instruction::UseModule { current_module } => {
                write!(f, "useModule (currently in {})", current_module)
            }
            Instruction::Panic => write!(f, "panic"),
            Instruction::ModuleStarts { module } => write!(f, "moduleStarts {module}"),
            Instruction::ModuleEnds => write!(f, "moduleEnds"),
            Instruction::TraceCallStarts { num_args } => {
                write!(f, "trace: callStarts ({num_args} args)")
            }
            Instruction::TraceCallEnds => write!(f, "trace: callEnds"),
            Instruction::TraceExpressionEvaluated => {
                write!(f, "trace: expressionEvaluated")
            }
            Instruction::TraceFoundFuzzableClosure => {
                write!(f, "trace: foundFuzzableClosure")
            }
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
