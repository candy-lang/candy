use crate::{
    heap::{
        Data, DisplayWithSymbolTable, Function, HirId, InlineObject, List, Pointer, Struct,
        SymbolTable, Tag, Text,
    },
    lir::Instruction,
    tracer::Tracer,
    vm::{CallHandle, MachineState, Panic},
};
use extension_trait::extension_trait;
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
        instruction: &Instruction,
        symbol_table: &SymbolTable,
        tracer: &mut impl Tracer,
    ) -> InstructionResult {
        if TRACE {
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
            trace!("Heap: {:?}", self.heap);
        }

        match instruction {
            Instruction::CreateTag { symbol_id } => {
                let value = self.pop_from_data_stack();
                let tag = Tag::create_with_value(&mut self.heap, true, *symbol_id, value);
                self.push_to_data_stack(tag);
                InstructionResult::Done
            }
            Instruction::CreateList { num_items } => {
                let mut item_addresses = vec![];
                for _ in 0..*num_items {
                    item_addresses.push(self.pop_from_data_stack());
                }
                let items = item_addresses.into_iter().rev().collect_vec();
                let list = List::create(&mut self.heap, true, &items);
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
                let struct_ = Struct::create(&mut self.heap, true, &entries);
                self.push_to_data_stack(struct_);
                InstructionResult::Done
            }
            Instruction::CreateFunction {
                captured,
                num_args,
                body,
            } => {
                let captured = captured
                    .iter()
                    .map(|offset| {
                        let object = self.get_from_data_stack(*offset);
                        object.dup(&mut self.heap);
                        object
                    })
                    .collect_vec();
                let function = Function::create(&mut self.heap, true, &captured, *num_args, *body);
                self.push_to_data_stack(function);
                InstructionResult::Done
            }
            Instruction::PushConstant(constant) => {
                self.push_to_data_stack(*constant);
                InstructionResult::Done
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(*offset);
                address.dup(&mut self.heap);
                self.push_to_data_stack(address);
                InstructionResult::Done
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.pop_from_data_stack();
                for _ in 0..*n {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }
                self.push_to_data_stack(top);
                InstructionResult::Done
            }
            Instruction::Call { num_args } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Build the reverse list in place.
                arguments.reverse();
                let callee = self.pop_from_data_stack();

                self.call(callee, &arguments, responsible, symbol_table)
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Built the reverse list in place
                arguments.reverse();
                let callee = self.pop_from_data_stack();
                for _ in 0..*num_locals_to_pop {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }

                // Tail calling a function is basically just a normal call, but
                // pretending we are our caller.
                self.next_instruction = self.call_stack.pop();
                self.call(callee, &arguments, responsible, symbol_table)
            }
            Instruction::Return => {
                self.next_instruction = self.call_stack.pop();
                InstructionResult::Done
            }
            Instruction::Panic => {
                let responsible_for_panic = self.pop_from_data_stack();
                let reason = self.pop_from_data_stack();

                let Ok(reason) = Text::try_from(reason) else {
                    // Panic expressions only occur inside the needs function
                    // where we have validated the inputs before calling the
                    // instructions, or when lowering compiler errors from the
                    // HIR to the MIR.
                    panic!("We should never generate a LIR where the reason is not a text.");
                };
                let responsible: HirId = responsible_for_panic.try_into().unwrap();

                InstructionResult::Panic(Panic {
                    reason: reason.get().to_owned(),
                    responsible: responsible.get().to_owned(),
                })
            }
            Instruction::TraceCallStarts { num_args } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut args = vec![];
                for _ in 0..*num_args {
                    args.push(self.pop_from_data_stack());
                }
                let callee = self.pop_from_data_stack();
                let call_site = self.pop_from_data_stack().try_into().unwrap();

                args.reverse();
                tracer.call_started(&mut self.heap, call_site, callee, args, responsible);
                InstructionResult::Done
            }
            Instruction::TraceCallEnds => {
                let return_value = self.pop_from_data_stack();

                tracer.call_ended(&mut self.heap, return_value);
                InstructionResult::Done
            }
            Instruction::TraceExpressionEvaluated => {
                let value = self.pop_from_data_stack();
                let expression = self.pop_from_data_stack().try_into().unwrap();

                tracer.value_evaluated(&mut self.heap, expression, value);
                InstructionResult::Done
            }
            Instruction::TraceFoundFuzzableFunction => {
                let function = self.pop_from_data_stack().try_into().expect(
                "Instruction TraceFoundFuzzableFunction executed, but stack top is not a function.",
            );
                let definition = self.pop_from_data_stack().try_into().unwrap();

                tracer.found_fuzzable_function(&mut self.heap, definition, function);
                InstructionResult::Done
            }
        }
    }

    pub fn call(
        &mut self,
        callee: InlineObject,
        arguments: &[InlineObject],
        responsible: HirId,
        symbol_table: &SymbolTable,
    ) -> InstructionResult {
        match callee.into() {
            Data::Function(function) => self.call_function(function, arguments, responsible),
            Data::Builtin(builtin) => {
                callee.drop(&mut self.heap);
                self.run_builtin_function(builtin.get(), arguments, responsible, symbol_table)
            }
            Data::Handle(handle) => {
                if arguments.len() != handle.argument_count() {
                    return InstructionResult::Panic(Panic {
                        reason: format!(
                            "A function expected {} parameters, but you called it with {} arguments.",
                            handle.argument_count(),
                            arguments.len(),
                        ),
                        responsible: responsible.get().clone(),
                    });
                }
                InstructionResult::CallHandle(CallHandle {
                handle,
                arguments: arguments.to_vec(),
                responsible,
            })
        },
            Data::Tag(tag) => {
                if tag.has_value() {
                    return InstructionResult::Panic(Panic {
                        reason: "A tag's value cannot be overwritten by calling it. Use `tag.withValue` instead.".to_string(),
                        responsible: responsible.get().to_owned(),
                    });
                }

                if let [value] = arguments {
                    let tag = Tag::create_with_value(&mut self.heap, true, tag.symbol_id(), *value);
                    self.push_to_data_stack(tag);
                    value.dup(&mut self.heap);
                    InstructionResult::Done
                } else {
                    InstructionResult::Panic(Panic {
                        reason: format!(
                            "A tag can only hold exactly one value, but you called it with {} arguments.",
                            arguments.len(),
                        ),
                        responsible: responsible.get().to_owned(),
                })
                }
            }
            _ => InstructionResult::Panic(Panic {
                reason: format!(
                    "You can only call functions, builtins, tags, and handles, but you tried to call {}.",
                    DisplayWithSymbolTable::to_string(&callee, symbol_table),
                ),
                responsible: responsible.get().to_owned(),
            }),
        }
    }
    pub fn call_function(
        &mut self,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
    ) -> InstructionResult {
        let expected_num_args = function.argument_count();
        if arguments.len() != expected_num_args {
            return InstructionResult::Panic(Panic {
                reason: format!(
                    "A function expected {expected_num_args} parameters, but you called it with {} arguments.",
                    arguments.len(),
                ),
                responsible: responsible.get().clone(),
            });
        }

        if let Some(next_instruction) = self.next_instruction {
            self.call_stack.push(next_instruction);
        }
        let captured = function.captured();
        for captured in captured {
            captured.dup(&mut self.heap);
        }
        self.data_stack.extend_from_slice(captured);
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
}

#[extension_trait]
impl NthLast for Vec<Pointer> {
    fn nth_last(&mut self, index: usize) -> Pointer {
        self[self.len() - 1 - index]
    }
}
