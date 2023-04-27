use crate::utils::DebugDisplay;
use candy_frontend::{
    builtin_functions::BuiltinFunction,
    hir,
    mir::Id,
    module::Module,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use itertools::Itertools;
use num_bigint::BigInt;
use std::fmt::{self, Display, Formatter};
use strum::{EnumDiscriminants, IntoStaticStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lir {
    pub instructions: Vec<Instruction>,
}

pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, EnumDiscriminants, Eq, Hash, IntoStaticStr, PartialEq)]
#[strum_discriminants(derive(Hash, IntoStaticStr), strum(serialize_all = "camelCase"))]
pub enum Instruction {
    /// Pushes an int.
    CreateInt(BigInt),

    /// Pushes a text.
    CreateText(String),

    /// Pushes an empty tag.
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

impl ToRichIr for Lir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let mut iterator = self.instructions.iter();
        if let Some(instruction) = iterator.next() {
            instruction.build_rich_ir(builder);
        }
        for instruction in iterator {
            builder.push_newline();
            instruction.build_rich_ir(builder);
        }
    }
}
impl ToRichIr for Instruction {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let discriminant: InstructionDiscriminants = self.into();
        builder.push(
            Into::<&'static str>::into(discriminant),
            None,
            EnumSet::empty(),
        );

        match self {
            Instruction::CreateInt(int) => {
                builder.push(" ", None, EnumSet::empty());
                let range = builder.push(int.to_string(), TokenType::Int, EnumSet::empty());
                builder.push_reference(int.to_owned(), range);
            }
            Instruction::CreateText(text) => {
                builder.push(" ", None, EnumSet::empty());
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.to_owned(), range);
            }
            Instruction::CreateSymbol(symbol) => {
                builder.push(" ", None, EnumSet::empty());
                let range = builder.push(symbol, TokenType::Text, EnumSet::empty());
                builder.push_reference(symbol.to_owned(), range);
            }
            Instruction::CreateList { num_items } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_items.to_string(), None, EnumSet::empty());
            }
            Instruction::CreateStruct { num_fields } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_fields.to_string(), None, EnumSet::empty());
            }
            Instruction::CreateHirId(id) => {
                builder.push(" ", None, EnumSet::empty());
                let range = builder.push(id.to_string(), None, EnumSet::empty());
                builder.push_reference(id.to_owned(), range);
            }
            Instruction::CreateClosure {
                captured,
                num_args,
                body,
            } => {
                builder.push(
                    format!(
                        " with {num_args} {} capturing {}",
                        arguments_plural(*num_args),
                        if captured.is_empty() {
                            "nothing".to_string()
                        } else {
                            captured.iter().join(", ")
                        },
                    ),
                    None,
                    EnumSet::empty(),
                );
                builder.push_foldable(|builder| {
                    builder.push_children_multiline(body);
                });
            }
            Instruction::CreateBuiltin(builtin_function) => {
                builder.push(" ", None, EnumSet::empty());
                let range = builder.push(format!("{:?}", builtin_function), None, EnumSet::empty());
                builder.push_reference(*builtin_function, range);
            }
            Instruction::PushFromStack(offset) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(offset.to_string(), None, EnumSet::empty());
            }
            Instruction::PopMultipleBelowTop(count) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(count.to_string(), None, EnumSet::empty());
            }
            Instruction::Call { num_args } => {
                builder.push(
                    format!(" with {num_args} {}", arguments_plural(*num_args),),
                    None,
                    EnumSet::empty(),
                );
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                builder.push(
                    format!(
                        " with {num_locals_to_pop} locals and {num_args} {}",
                        arguments_plural(*num_args),
                    ),
                    None,
                    EnumSet::empty(),
                );
            }
            Instruction::Return => {}
            Instruction::UseModule { current_module } => {
                builder.push(" (currently in ", None, EnumSet::empty());
                current_module.build_rich_ir(builder);
                builder.push(")", None, EnumSet::empty());
            }
            Instruction::Panic => {}
            Instruction::ModuleStarts { module } => {
                builder.push(" ", None, EnumSet::empty());
                module.build_rich_ir(builder);
            }
            Instruction::ModuleEnds => {}
            Instruction::TraceCallStarts { num_args } => {
                builder.push(
                    format!(" ({num_args} {})", arguments_plural(*num_args),),
                    None,
                    EnumSet::empty(),
                );
            }
            Instruction::TraceCallEnds => {}
            Instruction::TraceExpressionEvaluated => {}
            Instruction::TraceFoundFuzzableClosure => {}
        }
    }
}

impl DebugDisplay for Instruction {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        if is_debug {
            write!(f, "{:?}", self)
        } else {
            write!(f, "{}", self.to_rich_ir().text)
        }
    }
}
impl Display for Instruction {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        DebugDisplay::fmt(self, f, false)
    }
}

fn arguments_plural(num_args: usize) -> &'static str {
    if num_args == 1 {
        "argument"
    } else {
        "arguments"
    }
}
