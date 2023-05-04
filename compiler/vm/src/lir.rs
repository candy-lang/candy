use crate::heap::{Closure, HirId, InlineObject};
use crate::utils::DebugDisplay;
use crate::{fiber::InstructionPointer, heap::Heap};
use candy_frontend::{
    mir::Id,
    module::Module,
    rich_ir::{RichIr, RichIrBuilder, ToRichIr, TokenType},
    TracingConfig,
};
use enumset::EnumSet;
use extension_trait::extension_trait;
use itertools::Itertools;
use std::fmt::{self, Display, Formatter};
use strum::{EnumDiscriminants, IntoStaticStr};

pub struct Lir {
    pub module: Module,
    pub constant_heap: Heap,
    pub instructions: Vec<Instruction>,
    pub module_closure: Closure,
    pub responsible_module: HirId,
}

pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, EnumDiscriminants, Eq, Hash, IntoStaticStr, PartialEq)]
#[strum_discriminants(derive(Hash, IntoStaticStr), strum(serialize_all = "camelCase"))]
pub enum Instruction {
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

    /// Pushes a closure.
    ///
    /// a -> a, pointer to closure
    CreateClosure {
        captured: Vec<StackOffset>,
        num_args: usize, // excluding responsible parameter
        body: InstructionPointer,
    },

    /// Pushes a pointer onto the stack. MIR instructions that create
    /// compile-time known values are compiled to this instruction.
    PushConstant(InlineObject),

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

    /// Panics. Because the panic instruction only occurs inside the generated
    /// needs function, the reason is already guaranteed to be a text.
    ///
    /// a, reason, responsible -> 💥
    Panic,

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
            Instruction::CreateList { num_items } => {
                stack.pop_multiple(*num_items);
                stack.push(result);
            }
            Instruction::CreateStruct { num_fields } => {
                stack.pop_multiple(2 * num_fields); // fields
                stack.push(result);
            }
            Instruction::CreateClosure { .. } => {
                stack.push(result);
            }
            Instruction::PushConstant(_) => {
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
            Instruction::Panic => {
                stack.pop(); // responsible
                stack.pop(); // reason
                stack.push(result);
            }
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
            Instruction::CreateList { num_items } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_items.to_string(), None, EnumSet::empty());
            }
            Instruction::CreateStruct { num_fields } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_fields.to_string(), None, EnumSet::empty());
            }
            Instruction::CreateClosure {
                captured,
                num_args,
                body,
            } => {
                builder.push(
                    format!(
                        " with {num_args} {} capturing {} starting at {body:?}",
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
            }
            Instruction::PushConstant(constant) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(format!("{constant}"), TokenType::Address, EnumSet::empty());
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
            Instruction::Panic => {}
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

#[extension_trait]
pub impl RichIrForLir for RichIr {
    fn for_lir(module: &Module, lir: &Lir, tracing_config: &TracingConfig) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# LIR for module {}", module.to_rich_ir()),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        builder.push_tracing_config(tracing_config);
        builder.push_newline();
        lir.build_rich_ir(&mut builder);
        builder.finish()
    }
}
