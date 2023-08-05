use crate::{
    heap::{Function, Handle, Heap, HirId, InlineObject},
    instruction_pointer::InstructionPointer,
    instructions::InstructionResult,
    lir::Lir,
    tracer::Tracer,
};
use candy_frontend::hir::Id;
use derive_more::Deref;
use extension_trait::extension_trait;
use std::{borrow::Borrow, collections::HashMap, fmt::Debug, hash::Hash};

/// A VM represents a Candy program that thinks it's currently running. Because
/// VMs are first-class Rust structs, they enable other code to store "freezed"
/// programs and to remain in control about when and for how long code runs.
pub struct Vm<L: Borrow<Lir>, T: Tracer> {
    pub lir: L,
    state: MachineState,
    pub tracer: T,
}
pub(super) struct MachineState {
    pub next_instruction: Option<InstructionPointer>,
    pub data_stack: Vec<InlineObject>,
    pub call_stack: Vec<InstructionPointer>,
    pub heap: Heap,
}

pub struct CallHandle {
    pub handle: Handle,
    pub arguments: Vec<InlineObject>,
    pub responsible: HirId,
}

#[derive(Clone, Debug)]
pub struct Panic {
    pub reason: String,
    pub responsible: Id,
}

impl<L, T> Vm<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    pub fn for_function(
        lir: L,
        mut heap: Heap,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
        mut tracer: T,
    ) -> Self {
        tracer.call_started(
            &mut heap,
            responsible,
            function.into(),
            arguments.to_vec(),
            responsible,
        );

        let mut state = MachineState {
            next_instruction: None,
            data_stack: vec![],
            call_stack: vec![],
            heap,
        };
        state.call_function(function, arguments, responsible);

        Self { lir, state, tracer }
    }
    pub fn for_module(lir: L, tracer: T) -> Self {
        let actual_lir = lir.borrow();
        let function = actual_lir.module_function;
        let responsible = actual_lir.responsible_module;
        assert_eq!(
            function.captured_len(),
            0,
            "Function is not a module function (it captures stuff).",
        );
        assert_eq!(
            function.argument_count(),
            0,
            "Function is not a module function (it has arguments).",
        );
        Self::for_function(lir, Heap::default(), function, &[], responsible, tracer)
    }

    pub fn next_instruction(&self) -> Option<InstructionPointer> {
        self.state.next_instruction
    }
    pub fn call_stack(&self) -> &[InstructionPointer] {
        &self.state.call_stack
    }
    pub fn heap(&self) -> &Heap {
        &self.state.heap
    }
}

#[derive(Deref)]
pub struct VmHandleCall<L: Borrow<Lir>, T: Tracer> {
    #[deref]
    pub call: CallHandle,
    vm: Vm<L, T>,
}
pub struct VmReturned<T: Tracer> {
    pub heap: Heap,
    pub tracer: T,
    pub return_value: InlineObject,
}
pub struct VmPanicked<T: Tracer> {
    pub heap: Heap,
    pub tracer: T,
    pub panic: Panic,
}

pub enum StateAfterRun<L: Borrow<Lir>, T: Tracer> {
    Running(Vm<L, T>),
    CallingHandle(VmHandleCall<L, T>),
    Returned(VmReturned<T>),
    Panicked(VmPanicked<T>),
}

impl<L, T> VmHandleCall<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.vm.state.heap
    }

    pub fn complete(mut self, return_value: InlineObject) -> Vm<L, T> {
        self.vm.state.data_stack.push(return_value);
        self.vm
    }
}

impl<L, T> Vm<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    /// Runs one instruction in the VM and returns its new state.
    pub fn run(mut self) -> StateAfterRun<L, T> {
        let Some(current_instruction) = self.state.next_instruction else {
            let return_value = self.state.data_stack.pop().unwrap();
            self.tracer.call_ended(&mut self.state.heap, return_value);
            return StateAfterRun::Returned(VmReturned {
                heap: self.state.heap,
                tracer: self.tracer,
                return_value,
            });
        };

        let instruction = self
            .lir
            .borrow()
            .instructions
            .get(*current_instruction)
            .expect("invalid instruction pointer");
        self.state.next_instruction = Some(current_instruction.next());

        let result = self.state.run_instruction(
            instruction,
            &self.lir.borrow().symbol_table,
            &mut self.tracer,
        );
        match result {
            InstructionResult::Done => StateAfterRun::Running(self),
            InstructionResult::CallHandle(call) => {
                StateAfterRun::CallingHandle(VmHandleCall { vm: self, call })
            }
            InstructionResult::Panic(panic) => StateAfterRun::Panicked(VmPanicked {
                heap: self.state.heap,
                tracer: self.tracer,
                panic,
            }),
        }
    }

    /// Runs at most `max_instructions` in the VM.
    pub fn run_n(mut self, max_instructions: usize) -> StateAfterRun<L, T> {
        for _ in 0..max_instructions {
            match self.run() {
                StateAfterRun::Running(vm) => self = vm,
                a => return a,
            }
        }
        StateAfterRun::Running(self)
    }
}

pub enum StateAfterRunForever<L: Borrow<Lir>, T: Tracer> {
    CallingHandle(VmHandleCall<L, T>),
    Returned(VmReturned<T>),
    Panicked(VmPanicked<T>),
}

impl<L, T> Vm<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    /// Runs the VM until a handle call is performed, the VM returns, or it
    /// panics.
    pub fn run_forever(mut self) -> StateAfterRunForever<L, T> {
        loop {
            match self.run() {
                StateAfterRun::Running(vm) => self = vm,
                StateAfterRun::CallingHandle(call) => {
                    break StateAfterRunForever::CallingHandle(call)
                }
                StateAfterRun::Returned(returned) => {
                    break StateAfterRunForever::Returned(returned)
                }
                StateAfterRun::Panicked(panicked) => {
                    break StateAfterRunForever::Panicked(panicked)
                }
            }
        }
    }
}

pub enum StateAfterRunWithoutHandles<L: Borrow<Lir>, T: Tracer> {
    Running(Vm<L, T>),
    Returned(VmReturned<T>),
    Panicked(VmPanicked<T>),
}
impl<L, T> StateAfterRunWithoutHandles<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    fn unwrap_from(state: StateAfterRun<L, T>) -> Self {
        match state {
            StateAfterRun::Running(vm) => Self::Running(vm),
            StateAfterRun::CallingHandle(_) => panic!("A handle was called."),
            StateAfterRun::Returned(returned) => Self::Returned(returned),
            StateAfterRun::Panicked(panicked) => Self::Panicked(panicked),
        }
    }
}
impl<L, T> Vm<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    pub fn run_without_handles(self) -> StateAfterRunWithoutHandles<L, T> {
        StateAfterRunWithoutHandles::unwrap_from(self.run())
    }

    pub fn run_n_without_handles(
        self,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<L, T> {
        StateAfterRunWithoutHandles::unwrap_from(self.run_n(max_instructions))
    }
}

impl<L, T> Vm<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    /// Runs this VM until completion. Only call this if you are sure the VM
    /// won't call any handles.
    pub fn run_forever_without_handles(self) -> (Heap, T, Result<InlineObject, Panic>) {
        match self.run_forever() {
            StateAfterRunForever::CallingHandle(_) => {
                panic!("A handle was called.")
            }
            StateAfterRunForever::Returned(VmReturned {
                heap,
                tracer,
                return_value,
            }) => (heap, tracer, Ok(return_value)),
            StateAfterRunForever::Panicked(VmPanicked {
                heap,
                tracer,
                panic,
            }) => (heap, tracer, Err(panic)),
        }
    }
}

#[extension_trait]
impl<K: Eq + Hash, V> ReplaceHashMapValue<K, V> for HashMap<K, V> {
    fn replace<F: FnOnce(V) -> V>(&mut self, key: K, replacer: F) {
        let value = self.remove(&key).unwrap();
        let value = replacer(value);
        self.insert(key, value);
    }
}
