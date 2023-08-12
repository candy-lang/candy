use crate::{
    heap::{Function, Handle, Heap, InlineObject},
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
    // For type-safety, the VM has an API that takes ownership of the VM and
    // returns a new VM. If the VM is big, this causes lots of memcopies of
    // stack memory. So, we instead only store a pointer to the actual VM state.
    inner: Box<VmInner<L, T>>,
}

struct VmInner<L: Borrow<Lir>, T: Tracer> {
    lir: L,
    state: MachineState,
    tracer: T,
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
        mut tracer: T,
    ) -> Self {
        tracer.call_started(&mut heap, function.into(), arguments.to_vec());

        let mut state = MachineState {
            next_instruction: None,
            data_stack: vec![],
            call_stack: vec![],
            heap,
        };
        state.call_function(function, arguments);

        let inner = Box::new(VmInner { lir, state, tracer });
        Self { inner }
    }
    pub fn for_module(lir: L, tracer: T) -> Self {
        let actual_lir = lir.borrow();
        let function = actual_lir.module_function;
        assert_eq!(
            function.captured_len(),
            0,
            "Function is not a module function because it captures stuff.",
        );
        assert_eq!(
            function.argument_count(),
            0,
            "Function is not a module function because it has arguments.",
        );
        Self::for_function(lir, Heap::default(), function, &[], tracer)
    }

    pub fn lir(&self) -> &L {
        &self.inner.lir
    }
    pub fn tracer(&self) -> &T {
        &self.inner.tracer
    }
    pub fn next_instruction(&self) -> Option<InstructionPointer> {
        self.inner.state.next_instruction
    }
    pub fn call_stack(&self) -> &[InstructionPointer] {
        &self.inner.state.call_stack
    }
    pub fn heap(&self) -> &Heap {
        &self.inner.state.heap
    }
}

#[derive(Deref)]
pub struct VmHandleCall<L: Borrow<Lir>, T: Tracer> {
    #[deref]
    pub call: CallHandle,
    vm: Vm<L, T>,
}
pub struct VmFinished<T: Tracer> {
    pub heap: Heap,
    pub tracer: T,
    pub result: Result<InlineObject, Panic>,
}

pub enum StateAfterRun<L: Borrow<Lir>, T: Tracer> {
    Running(Vm<L, T>),
    CallingHandle(VmHandleCall<L, T>),
    Finished(VmFinished<T>),
}

impl<L, T> VmHandleCall<L, T>
where
    L: Borrow<Lir>,
    T: Tracer,
{
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.vm.inner.state.heap
    }

    pub fn complete(mut self, return_value: impl Into<InlineObject>) -> Vm<L, T> {
        self.vm.inner.state.data_stack.push(return_value.into());
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
        let Some(current_instruction) = self.inner.state.next_instruction else {
            let return_value = self.inner.state.data_stack.pop().unwrap();
            self.inner
                .tracer
                .call_ended(&mut self.inner.state.heap, return_value);
            return StateAfterRun::Finished(VmFinished {
                heap: self.inner.state.heap,
                tracer: self.inner.tracer,
                result: Ok(return_value),
            });
        };

        let instruction = self
            .inner
            .lir
            .borrow()
            .instructions
            .get(*current_instruction)
            .expect("invalid instruction pointer");
        self.inner.state.next_instruction = Some(current_instruction.next());

        let result = self.inner.state.run_instruction(
            instruction,
            &self.inner.lir.borrow().symbol_table,
            &mut self.inner.tracer,
        );
        match result {
            InstructionResult::Done => StateAfterRun::Running(self),
            InstructionResult::CallHandle(call) => {
                StateAfterRun::CallingHandle(VmHandleCall { vm: self, call })
            }
            InstructionResult::Panic(panic) => StateAfterRun::Finished(VmFinished {
                heap: self.inner.state.heap,
                tracer: self.inner.tracer,
                result: Err(panic),
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
    Finished(VmFinished<T>),
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
                StateAfterRun::Finished(finished) => {
                    break StateAfterRunForever::Finished(finished)
                }
            }
        }
    }
}

pub enum StateAfterRunWithoutHandles<L: Borrow<Lir>, T: Tracer> {
    Running(Vm<L, T>),
    Finished(VmFinished<T>),
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
            StateAfterRun::Finished(finished) => Self::Finished(finished),
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
    pub fn run_forever_without_handles(self) -> VmFinished<T> {
        match self.run_forever() {
            StateAfterRunForever::CallingHandle(_) => {
                panic!("A handle was called.")
            }
            StateAfterRunForever::Finished(finished) => finished,
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
