use crate::heap::Heap;
use crate::heap::{
    DisplayWithSymbolTable, Function, HirId, InlineData, InlineObject, SymbolId, SymbolTable,
};
use crate::instruction_pointer::InstructionPointer;
use candy_frontend::hir;
use candy_frontend::rich_ir::ReferenceKey;
use candy_frontend::{
    mir::Id,
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

pub struct Lir {
    pub module: Module,
    pub constant_heap: Heap,
    pub symbol_table: SymbolTable,
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
    CreateTag {
        symbol_id: SymbolId,
    },

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

    /// Pushes a function.
    ///
    /// a -> a, pointer to function
    CreateFunction {
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
        num_args: usize, // excluding the responsible argument
    },

    /// Returns from the current function to the original caller. Leaves the
    /// data stack untouched, but pops a caller from the call stack and returns
    /// the instruction pointer to continue where the current function was
    /// called.
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

    /// a, HIR ID, function -> a
    TraceFoundFuzzableFunction,
}

impl Instruction {
    /// Applies the instruction's effect on the stack. After calling it, the
    /// stack will be in the same state as when the control flow continues after
    /// this instruction.
    pub fn apply_to_stack(&self, stack: &mut Vec<Id>, result: Id) {
        match self {
            Instruction::CreateTag { .. } => {
                stack.pop();
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
            Instruction::CreateFunction { .. } => {
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
                stack.pop(); // function/builtin
                stack.push(result); // return value
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                stack.pop(); // responsible
                stack.pop_multiple(*num_args);
                stack.pop(); // function/builtin
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
            Instruction::TraceFoundFuzzableFunction => {
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

impl Lir {
    pub fn functions_behind(&self, ip: InstructionPointer) -> &FxHashSet<hir::Id> {
        &self.origins[*ip]
    }
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

impl ToRichIr for Lir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push("# Symbol Table", TokenType::Comment, EnumSet::empty());
        for (symbol_id, symbol) in self.symbol_table.ids_and_symbols() {
            builder.push_newline();
            builder.push(
                format!("{:?}", symbol_id),
                TokenType::Address,
                EnumSet::empty(),
            );
            builder.push(": ", None, EnumSet::empty());
            let symbol_range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
            builder.push_reference(ReferenceKey::Symbol(symbol.to_string()), symbol_range);
        }
        builder.push_newline();
        builder.push_newline();

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
                    ToString::to_string(&i)
                        .pad_to_width_with_alignment(instruction_index_width, Alignment::Right),
                ),
                TokenType::Comment,
                EnumSet::empty(),
            );

            instruction.build_rich_ir(builder, &self.symbol_table);
        }
    }
}
impl Instruction {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder, symbol_table: &SymbolTable) {
        let discriminant: InstructionDiscriminants = self.into();
        builder.push(
            Into::<&'static str>::into(discriminant),
            None,
            EnumSet::empty(),
        );

        match self {
            Instruction::CreateTag { symbol_id } => {
                builder.push(" ", None, EnumSet::empty());
                let symbol_range = builder.push(
                    DisplayWithSymbolTable::to_string(symbol_id, symbol_table),
                    None,
                    EnumSet::empty(),
                );
                builder.push_reference(
                    ReferenceKey::Symbol(symbol_table.get(*symbol_id).to_string()),
                    symbol_range,
                );
            }
            Instruction::CreateList { num_items } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(ToString::to_string(num_items), None, EnumSet::empty());
            }
            Instruction::CreateStruct { num_fields } => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(ToString::to_string(num_fields), None, EnumSet::empty());
            }
            Instruction::CreateFunction {
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
            Instruction::PushFromStack(offset) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(ToString::to_string(offset), None, EnumSet::empty());
            }
            Instruction::PopMultipleBelowTop(count) => {
                builder.push(" ", None, EnumSet::empty());
                builder.push(ToString::to_string(count), None, EnumSet::empty());
            }
            Instruction::Call { num_args } => {
                builder.push(
                    format!(" with {num_args} {}", arguments_plural(*num_args)),
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
                    format!(" ({num_args} {})", arguments_plural(*num_args)),
                    None,
                    EnumSet::empty(),
                );
            }
            Instruction::TraceCallEnds => {}
            Instruction::TraceExpressionEvaluated => {}
            Instruction::TraceFoundFuzzableFunction => {}
        }
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
    fn for_byte_code(module: &Module, byte_code: &Lir, tracing_config: &TracingConfig) -> RichIr {
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
