use crate::{
    byte_code::{CreateFunction, IfElse, Instruction},
    heap::{Data, Function, Heap, HirId, InlineObject, List, Struct, Tag, Text},
    tracer::Tracer,
    vm::{CallHandle, MachineState, Panic},
};
use itertools::Itertools;
use tracing::trace;

const TRACE: bool = false;

pub enum InstructionResult {
    Done,
    CallHandle(CallHandle),
    Panic(Panic),
}

impl MachineState {
    pub fn run_instruction(
        &mut self,
        heap: &mut Heap,
        instruction: &Instruction,
        tracer: &mut impl Tracer,
    ) -> InstructionResult {
        if TRACE {
            trace!("");
            trace!("Running instruction: {instruction:?}");
            trace!("Instruction pointer: {:?}", self.next_instruction.unwrap());
            trace!(
                "Data stack: {}",
                if self.data_stack.is_empty() {
                    "<empty>".to_string()
                } else {
                    self.data_stack
                        .iter()
                        .map(|it| format!("{it:?}"))
                        .join(", ")
                },
            );
            trace!(
                "Call stack: {}",
                if self.call_stack.is_empty() {
                    "<empty>".to_string()
                } else {
                    self.call_stack
                        .iter()
                        .map(|ip| format!("{ip:?}"))
                        .join(", ")
                },
            );
            trace!("Heap: {heap:?}");
        }

        match instruction {
            Instruction::CreateTag { symbol } => {
                let value = self.pop_from_data_stack();
                let tag = Tag::create_with_value(heap, true, *symbol, value);
                self.push_to_data_stack(tag);
                InstructionResult::Done
            }
            Instruction::CreateList { num_items } => {
                let mut item_addresses = vec![];
                for _ in 0..*num_items {
                    item_addresses.push(self.pop_from_data_stack());
                }
                let items = item_addresses.into_iter().rev().collect_vec();
                let list = List::create(heap, true, &items);
                self.push_to_data_stack(list);
                InstructionResult::Done
            }
            Instruction::CreateStruct { num_fields } => {
                // PERF: Avoid collecting keys and values into a `Vec` before creating the `HashMap`
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_fields) {
                    key_value_addresses.push(self.pop_from_data_stack());
                }
                let entries = key_value_addresses.into_iter().rev().tuples().collect();
                let struct_ = Struct::create(heap, true, &entries);
                self.push_to_data_stack(struct_);
                InstructionResult::Done
            }
            Instruction::CreateFunction(box CreateFunction {
                captured,
                num_args,
                body,
            }) => {
                let captured = captured
                    .iter()
                    .map(|offset| self.get_from_data_stack(*offset))
                    .collect_vec();
                let function = Function::create(heap, true, &captured, *num_args, *body);
                self.push_to_data_stack(function);
                InstructionResult::Done
            }
            Instruction::PushConstant(constant) => {
                self.push_to_data_stack(*constant);
                InstructionResult::Done
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(*offset);
                self.push_to_data_stack(address);
                InstructionResult::Done
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.pop_from_data_stack();
                self.pop_multiple_from_data_stack(*n);
                self.push_to_data_stack(top);
                InstructionResult::Done
            }
            Instruction::Dup { amount } => {
                self.pop_from_data_stack().dup_by(heap, *amount);
                InstructionResult::Done
            }
            Instruction::Drop => {
                self.pop_from_data_stack().drop(heap);
                InstructionResult::Done
            }
            Instruction::Call { num_args } => {
                let responsible = HirId::new_unchecked(self.pop_from_data_stack());
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Build the reverse list in place.
                arguments.reverse();
                let callee = self.pop_from_data_stack();

                self.call(heap, callee, &arguments, responsible)
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                let responsible = HirId::new_unchecked(self.pop_from_data_stack());
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Built the reverse list in place
                arguments.reverse();
                let callee = self.pop_from_data_stack();
                self.pop_multiple_from_data_stack(*num_locals_to_pop);

                // Tail calling a function is basically just a normal call, but
                // pretending we are our caller.
                self.next_instruction = self.call_stack.pop();
                self.call(heap, callee, &arguments, responsible)
            }
            Instruction::Return => {
                self.next_instruction = self.call_stack.pop();
                InstructionResult::Done
            }
            Instruction::IfElse(box IfElse {
                then_target,
                then_captured,
                else_target,
                else_captured,
            }) => {
                let responsible = self.pop_from_data_stack();
                let condition = Tag::value_into_bool_unchecked(self.pop_from_data_stack(), heap);
                let (target, captured) = if condition {
                    (*then_target, then_captured)
                } else {
                    (*else_target, else_captured)
                };

                if let Some(next_instruction) = self.next_instruction {
                    self.call_stack.push(next_instruction);
                }

                // Initially, we need to adjust the offset because we already
                // popped two values from the data stack. Afterwards, increment
                // it for each value.
                for (index, offset) in captured.iter().enumerate() {
                    let captured = self.get_from_data_stack(*offset - 2 + index);
                    self.data_stack.push(captured);
                }
                self.push_to_data_stack(responsible);
                self.next_instruction = Some(target);
                InstructionResult::Done
            }
            Instruction::Panic => {
                let responsible = HirId::new_unchecked(self.pop_from_data_stack());
                let reason = self.pop_from_data_stack();

                let Ok(reason) = Text::try_from(reason) else {
                    // Panic expressions only occur inside the needs function
                    // where we have validated the inputs before calling the
                    // instructions, or when lowering compiler errors from the
                    // HIR to the MIR.
                    panic!("We should never generate byte code where the reason is not a text.");
                };

                InstructionResult::Panic(Panic {
                    reason: reason.get().to_string(),
                    responsible: responsible.get().clone(),
                })
            }
            Instruction::TraceCallStarts { num_args } => {
                let responsible = HirId::new_unchecked(self.pop_from_data_stack());
                let mut args = vec![];
                for _ in 0..*num_args {
                    args.push(self.pop_from_data_stack());
                }
                let callee = self.pop_from_data_stack();
                let call_site = HirId::new_unchecked(self.pop_from_data_stack());

                args.reverse();
                tracer.call_started(heap, call_site, callee, args, responsible);
                InstructionResult::Done
            }
            Instruction::TraceCallEnds { has_return_value } => {
                let return_value = if *has_return_value {
                    Some(self.pop_from_data_stack())
                } else {
                    None
                };
                tracer.call_ended(heap, return_value);
                InstructionResult::Done
            }
            Instruction::TraceTailCall { num_args } => {
                let responsible = HirId::new_unchecked(self.pop_from_data_stack());
                let mut args = vec![];
                for _ in 0..*num_args {
                    args.push(self.pop_from_data_stack());
                }
                let callee = self.pop_from_data_stack();
                let call_site = HirId::new_unchecked(self.pop_from_data_stack());

                args.reverse();
                tracer.tail_call(heap, call_site, callee, args, responsible);
                InstructionResult::Done
            }
            Instruction::TraceExpressionEvaluated => {
                let value = self.pop_from_data_stack();
                let expression = HirId::new_unchecked(self.pop_from_data_stack());

                tracer.value_evaluated(heap, expression, value);
                InstructionResult::Done
            }
            Instruction::TraceFoundFuzzableFunction => {
                let function = self.pop_from_data_stack().try_into().expect(
                    "Instruction TraceFoundFuzzableFunction executed, but stack top is not a function.",
                );
                let definition = HirId::new_unchecked(self.pop_from_data_stack());

                tracer.found_fuzzable_function(heap, definition, function);
                InstructionResult::Done
            }
        }
    }

    pub fn call(
        &mut self,
        heap: &mut Heap,
        callee: InlineObject,
        arguments: &[InlineObject],
        responsible: HirId,
    ) -> InstructionResult {
        match callee.into() {
            Data::Function(function) => self.call_function(function, arguments, responsible),
            Data::Builtin(builtin) => {
                self.run_builtin_function(heap, builtin.get(), arguments, responsible)
            }
            Data::Handle(handle) => {
                debug_assert_eq!(handle.argument_count(), arguments.len());
                InstructionResult::CallHandle(CallHandle {
                    handle,
                    arguments: arguments.to_vec(),
                    responsible,
                })
            }
            _ => panic!("You can only call functions, builtins, and handles."),
        }
    }
    pub fn call_function(
        &mut self,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
    ) -> InstructionResult {
        debug_assert_eq!(function.argument_count(), arguments.len());
        if let Some(next_instruction) = self.next_instruction {
            self.call_stack.push(next_instruction);
        }
        self.data_stack.extend_from_slice(function.captured());
        self.data_stack.extend_from_slice(arguments);
        self.push_to_data_stack(responsible);
        self.next_instruction = Some(function.body());
        InstructionResult::Done
    }

    fn get_from_data_stack(&self, offset: usize) -> InlineObject {
        self.data_stack[self.data_stack.len() - 1 - offset]
    }
    fn push_to_data_stack(&mut self, value: impl Into<InlineObject>) {
        self.data_stack.push(value.into());
    }
    fn pop_from_data_stack(&mut self) -> InlineObject {
        self.data_stack.pop().expect("Data stack is empty.")
    }
    fn pop_multiple_from_data_stack(&mut self, amount: usize) {
        debug_assert!(amount <= self.data_stack.len());
        self.data_stack.truncate(self.data_stack.len() - amount);
    }
}
