use crate::heap::{Function, HirId, InlineData, InlineObject};
use crate::heap::{Heap, Text};
use crate::instruction_pointer::InstructionPointer;
use candy_frontend::hir;
use candy_frontend::rich_ir::ReferenceKey;
use candy_frontend::{
    lir::Id,
    module::Module,
    rich_ir::{RichIr, RichIrBuilder, ToRichIr, TokenType},
    TracingConfig,
};
use enumset::EnumSet;
use extension_trait::extension_trait;
use itertools::Itertools;
use pad::{Alignment, PadStr};
use rustc_hash::FxHashSet;
use std::ops::Range;
use strum::{EnumDiscriminants, IntoStaticStr};

pub struct ByteCode {
    pub module: Module,
    pub constant_heap: Heap,
    pub instructions: Vec<Instruction>,
    pub(super) origins: Vec<FxHashSet<hir::Id>>,
    pub module_function: Function,
    pub responsible_module: HirId,
}

pub type StackOffset = usize; // 0 is the last item, 1 the one before that, etc.

#[derive(Clone, Debug, EnumDiscriminants, Eq, Hash, IntoStaticStr, PartialEq)]
#[strum_discriminants(derive(Hash, IntoStaticStr), strum(serialize_all = "camelCase"))]
pub enum Instruction {
    /// Pops 1 argument, pushes a tag.
    ///
    /// a, value -> a, tag
    CreateTag { symbol: Text },

    /// Pops num_items items, pushes a list.
    ///
    /// a, item, item, ..., item -> a, pointer to list
    CreateList { num_items: usize },

    /// Pops 2 * num_fields items, pushes a struct.
    ///
    /// a, key, value, key, value, ..., key, value -> a, pointer to struct
    CreateStruct { num_fields: usize },

    /// Pushes a function.
    ///
    /// a -> a, pointer to function
    CreateFunction(Box<CreateFunction>),

    /// Pushes a pointer onto the stack. MIR instructions that create
    /// compile-time known values are compiled to this instruction.
    PushConstant(InlineObject),

    /// Pushes an item from back in the stack on the stack again.
    PushFromStack(StackOffset),

    /// Leaves the top stack item untouched, but removes n below.
    PopMultipleBelowTop(usize),

    /// Increases the reference count by `amount`.
    ///
    /// a, value -> a
    Dup { amount: usize },

    /// Decreases the reference count by one and, if the reference count reaches
    /// zero, deallocates it.
    ///
    /// a, value -> a
    Drop,

    /// Sets up the data stack for a function execution and then changes the
    /// instruction pointer to the first instruction.
    ///
    /// a, function, arg1, arg2, ..., argN, responsible -> a, caller, captured vars, arg1, arg2, ..., argN, responsible
    ///
    /// Later, when the function returns (perhaps many instructions after this
    /// one), the stack will contain the result:
    ///
    /// a, function, arg1, arg2, ..., argN, responsible ~> a, return value from function
    Call {
        num_args: usize, // excluding the responsible argument
    },

    /// Like `Call`, but after popping the stack entries for the call itself, it
    /// also pops the given number of local stack entries before actually
    /// executing the call.
    TailCall {
        num_locals_to_pop: usize,
        // This is `u32` instead of `usize` to reduce the size of the
        // enum from 24 to 16 bytes.
        num_args: u32, // excluding the responsible argument
    },

    /// Returns from the current function to the original caller. Leaves the
    /// data stack untouched, but pops a caller from the call stack and returns
    /// the instruction pointer to continue where the current function was
    /// called.
    Return,

    /// Conditionally calls either `then_target` or `else_target` depending on
    /// the `condition`.
    ///
    /// a, condition, responsible -> a
    IfElse(Box<IfElse>),
    // Optimization: Don't box the `IfElse` instruction if it doesn't capture
    // anything.
    IfElseWithoutCaptures {
        then_target: InstructionPointer,
        else_target: InstructionPointer,
    },

    /// Panics. Because the panic instruction only occurs inside the generated
    /// needs function, the reason is already guaranteed to be a text.
    ///
    /// a, reason, responsible -> 💥
    Panic,

    /// a, HIR ID, function, arg1, arg2, ..., argN, responsible -> a
    TraceCallStarts { num_args: usize },

    /// a -> a
    /// or:
    /// a, return value -> a
    TraceCallEnds { has_return_value: bool },

    /// a, HIR ID, function, arg1, arg2, ..., argN, responsible -> a
    TraceTailCall { num_args: usize },

    /// a, HIR ID, value -> a
    TraceExpressionEvaluated,

    /// a, HIR ID, function -> a
    TraceFoundFuzzableFunction,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CreateFunction {
    pub captured: Vec<StackOffset>,
    pub num_args: usize, // excluding responsible parameter
    pub body: InstructionPointer,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IfElse {
    pub then_target: InstructionPointer,
    pub then_captured: Vec<StackOffset>,
    pub else_target: InstructionPointer,
    pub else_captured: Vec<StackOffset>,
}

impl Instruction {
    /// Applies the instruction's effect on the stack. After calling it, the
    /// stack will be in the same state as when the control flow continues after
    /// this instruction.
    pub fn apply_to_stack(&self, stack: &mut Vec<Id>, result: Id) {
        match self {
            Self::CreateTag { .. } => {
                stack.pop();
                stack.push(result);
            }
            Self::CreateList { num_items } => {
                stack.pop_multiple(*num_items);
                stack.push(result);
            }
            Self::CreateStruct { num_fields } => {
                stack.pop_multiple(2 * num_fields); // fields
                stack.push(result);
            }
            Self::CreateFunction { .. } => {
                stack.push(result);
            }
            Self::PushConstant(_) => {
                stack.push(result);
            }
            Self::PushFromStack(_) => {
                stack.push(result);
            }
            Self::PopMultipleBelowTop(n) => {
                let top = stack.pop().unwrap();
                stack.pop_multiple(*n);
                stack.push(top);
            }
            Self::Dup { amount: _ } => {
                stack.pop();
            }
            Self::Drop => {
                stack.pop();
            }
            Self::Call { num_args } => {
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // function/builtin
                stack.push(result); // return value
            }
            Self::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                stack.pop(); // responsible
                stack.pop_multiple((*num_args).try_into().unwrap());
                stack.pop(); // function/builtin
                stack.pop_multiple(*num_locals_to_pop);
                stack.push(result); // return value
            }
            Self::Return => {
                // Only modifies the call stack and the instruction pointer.
                // Leaves the return value untouched on the stack.
            }
            Self::IfElse(_) | Self::IfElseWithoutCaptures { .. } => {
                stack.pop(); // responsible
                stack.pop(); // condition
                stack.push(result); // return value
            }
            Self::Panic => {
                stack.pop(); // responsible
                stack.pop(); // reason
                stack.push(result);
            }
            Self::TraceCallStarts { num_args } | Self::TraceTailCall { num_args } => {
                stack.pop(); // HIR ID
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // callee
            }
            Self::TraceCallEnds { has_return_value } => {
                if *has_return_value {
                    stack.pop(); // return value
                }
            }
            Self::TraceExpressionEvaluated => {
                stack.pop(); // HIR ID
                stack.pop(); // value
            }
            Self::TraceFoundFuzzableFunction => {
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

impl ByteCode {
    #[must_use]
    pub fn functions_behind(&self, ip: InstructionPointer) -> &FxHashSet<hir::Id> {
        &self.origins[*ip]
    }
    #[must_use]
    pub fn range_of_function(&self, function: &hir::Id) -> Range<InstructionPointer> {
        let start = self
            .origins
            .iter()
            .position(|origins| origins.contains(function))
            .unwrap();
        let end = start
            + self
                .origins
                .iter()
                .skip(start)
                .take_while(|origins| origins.contains(function))
                .count();
        start.into()..end.into()
    }
}

impl ToRichIr for ByteCode {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push("# Constant heap", TokenType::Comment, EnumSet::empty());
        for constant in self.constant_heap.iter() {
            builder.push_newline();
            builder.push(
                format!("{:p}", constant.address()),
                TokenType::Address,
                EnumSet::empty(),
            );
            builder.push(": ", None, EnumSet::empty());
            builder.push(
                format!("{constant:?}"),
                TokenType::Constant,
                EnumSet::empty(),
            );
        }
        builder.push_newline();
        builder.push_newline();

        builder.push("# Instructions", TokenType::Comment, EnumSet::empty());
        let instruction_index_width = (self.instructions.len() * 10 - 1).ilog10() as usize;
        let mut previous_origins = &FxHashSet::default();
        for (i, instruction) in self.instructions.iter().enumerate() {
            builder.push_newline();

            let origins = &self.origins[i];
            if origins != previous_origins {
                builder.push(
                    format!("# {}", origins.iter().join(", ")),
                    TokenType::Comment,
                    EnumSet::empty(),
                );
                builder.push_newline();
                previous_origins = origins;
            }

            builder.push(
                format!(
                    "{}: ",
                    i.to_string()
                        .pad_to_width_with_alignment(instruction_index_width, Alignment::Right),
                ),
                TokenType::Comment,
                EnumSet::empty(),
            );

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
            Self::CreateTag { symbol } => {
                builder.push(" ", None, EnumSet::empty());
                let symbol_range = builder.push(symbol.get(), None, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.to_string()), symbol_range);
            }
            Self::CreateList { num_items } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_items.to_string(), None, EnumSet::empty());
            }
            Self::CreateStruct { num_fields } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(num_fields.to_string(), None, EnumSet::empty());
            }
            Self::CreateFunction(box CreateFunction {
                captured,
                num_args,
                body,
            }) => {
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
            Self::PushConstant(constant) => {
                builder.push(" ", None, EnumSet::empty());
                if let InlineData::Pointer(pointer) = InlineData::from(*constant) {
                    builder.push(
                        format!("{:?}", pointer.get().address()),
                        TokenType::Address,
                        EnumSet::empty(),
                    );
                } else {
                    builder.push("inline", TokenType::Address, EnumSet::empty());
                }
                builder.push(" ", None, EnumSet::empty());
                builder.push(
                    format!("{constant:?}"),
                    TokenType::Constant,
                    EnumSet::empty(),
                );
            }
            Self::PushFromStack(offset) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(offset.to_string(), None, EnumSet::empty());
            }
            Self::PopMultipleBelowTop(count) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(count.to_string(), None, EnumSet::empty());
            }
            Self::Dup { amount } => {
                builder.push(" by ", None, EnumSet::empty());
                builder.push(amount.to_string(), None, EnumSet::empty());
            }
            Self::Drop => {}
            Self::Call { num_args } => {
                builder.push(
                    format!(" with {num_args} {}", arguments_plural(*num_args)),
                    None,
                    EnumSet::empty(),
                );
            }
            Self::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                builder.push(
                    format!(
                        " with {num_locals_to_pop} locals and {num_args} {}",
                        arguments_plural((*num_args).try_into().unwrap()),
                    ),
                    None,
                    EnumSet::empty(),
                );
            }
            Self::Return => {}
            Self::IfElse(box IfElse {
                then_target,
                then_captured,
                else_target,
                else_captured,
            }) => {
                builder.push(
                    format!(
                        " then call {then_target:?} capturing {} else call {else_target:?} capturing {}",
                        if then_captured.is_empty() {
                            "nothing".to_string()
                        } else {
                            then_captured.iter().join(", ")
                        },
                        if else_captured.is_empty() {
                            "nothing".to_string()
                        } else {
                            else_captured.iter().join(", ")
                        },
                    ),
                    None,
                     EnumSet::empty(),
                );
            }
            Self::IfElseWithoutCaptures {
                then_target,
                else_target,
            } => {
                builder.push(
                    format!(" then call {then_target:?} else call {else_target:?}"),
                    None,
                    EnumSet::empty(),
                );
            }
            Self::Panic => {}
            Self::TraceCallStarts { num_args } | Self::TraceTailCall { num_args } => {
                builder.push(
                    format!(" ({num_args} {})", arguments_plural(*num_args)),
                    None,
                    EnumSet::empty(),
                );
            }
            Self::TraceCallEnds { has_return_value } => {
                builder.push(
                    if *has_return_value {
                        " with return value"
                    } else {
                        " without return value"
                    },
                    None,
                    EnumSet::empty(),
                );
            }
            Self::TraceExpressionEvaluated => {}
            Self::TraceFoundFuzzableFunction => {}
        }
    }
}

const fn arguments_plural(num_args: usize) -> &'static str {
    if num_args == 1 {
        "argument"
    } else {
        "arguments"
    }
}

#[extension_trait]
pub impl RichIrForByteCode for RichIr {
    fn for_byte_code(
        module: &Module,
        byte_code: &ByteCode,
        tracing_config: TracingConfig,
    ) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# VM Byte Code for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        builder.push_tracing_config(tracing_config);
        builder.push_newline();
        byte_code.build_rich_ir(&mut builder);
        builder.finish(true)
    }
}
