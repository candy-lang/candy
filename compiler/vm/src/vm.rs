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
pub struct MachineState {
    pub next_instruction: Option<InstructionPointer>,
    pub data_stack: Vec<InlineObject>,
    pub call_stack: Vec<InstructionPointer>,
}

#[derive(Debug)]
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
        heap: &mut Heap,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
        mut tracer: T,
    ) -> Self {
        tracer.call_started(
            heap,
            responsible,
            function.into(),
            arguments.to_vec(),
            responsible,
        );

        let mut state = MachineState {
            next_instruction: None,
            data_stack: vec![],
            call_stack: vec![],
        };
        state.call_function(heap, function, arguments, responsible);

        let inner = Box::new(VmInner { lir, state, tracer });
        Self { inner }
    }
    pub fn for_module(lir: L, heap: &mut Heap, tracer: T) -> Self {
        let actual_lir = lir.borrow();
        let function = actual_lir.module_function;
        let responsible = actual_lir.responsible_module;
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
        Self::for_function(lir, heap, function, &[], responsible, tracer)
    }

    #[must_use]
    pub fn lir(&self) -> &L {
        &self.inner.lir
    }
    #[must_use]
    pub fn tracer(&self) -> &T {
        &self.inner.tracer
    }
    #[must_use]
    pub fn next_instruction(&self) -> Option<InstructionPointer> {
        self.inner.state.next_instruction
    }
    #[must_use]
    pub fn call_stack(&self) -> &[InstructionPointer] {
        &self.inner.state.call_stack
    }
}

#[derive(Deref)]
pub struct VmHandleCall<L: Borrow<Lir>, T: Tracer> {
    #[deref]
    pub call: CallHandle,
    vm: Vm<L, T>,
}
#[must_use]
pub struct VmFinished<T: Tracer> {
    pub tracer: T,
    pub result: Result<InlineObject, Panic>,
}

#[must_use]
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
    pub fn complete(mut self, heap: &mut Heap, return_value: impl Into<InlineObject>) -> Vm<L, T> {
        self.handle.drop(heap);
        for argument in &self.call.arguments {
            argument.drop(heap);
        }

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
    pub fn run(mut self, heap: &mut Heap) -> StateAfterRun<L, T> {
        let Some(current_instruction) = self.inner.state.next_instruction else {
            let return_value = self.inner.state.data_stack.pop().unwrap();
            self.inner.tracer.call_ended(heap, return_value);
            return StateAfterRun::Finished(VmFinished {
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

        let result = self
            .inner
            .state
            .run_instruction(heap, instruction, &mut self.inner.tracer);
        match result {
            InstructionResult::Done => StateAfterRun::Running(self),
            InstructionResult::CallHandle(call) => {
                StateAfterRun::CallingHandle(VmHandleCall { vm: self, call })
            }
            InstructionResult::Panic(panic) => StateAfterRun::Finished(VmFinished {
                tracer: self.inner.tracer,
                result: Err(panic),
            }),
        }
    }

    /// Runs at most `max_instructions` in the VM.
    pub fn run_n(mut self, heap: &mut Heap, max_instructions: usize) -> StateAfterRun<L, T> {
        for _ in 0..max_instructions {
            match self.run(heap) {
                StateAfterRun::Running(vm) => self = vm,
                a => return a,
            }
        }
        StateAfterRun::Running(self)
    }
}

#[must_use]
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
    pub fn run_forever(mut self, heap: &mut Heap) -> StateAfterRunForever<L, T> {
        loop {
            match self.run(heap) {
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

#[extension_trait]
impl<K: Eq + Hash, V> ReplaceHashMapValue<K, V> for HashMap<K, V> {
    fn replace<F: FnOnce(V) -> V>(&mut self, key: K, replacer: F) {
        let value = self.remove(&key).unwrap();
        let value = replacer(value);
        self.insert(key, value);
    }
}
