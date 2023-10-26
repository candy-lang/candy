use crate::{
    byte_code::ByteCode,
    heap::{Function, Handle, Heap, HirId, InlineObject, Struct},
    instruction_pointer::InstructionPointer,
    instructions::InstructionResult,
    tracer::Tracer,
};
use candy_frontend::hir::{self, Id};
use derive_more::Deref;
use extension_trait::extension_trait;
use std::{borrow::Borrow, collections::HashMap, fmt::Debug, hash::Hash};

/// A VM represents a Candy program that thinks it's currently running. Because
/// VMs are first-class Rust structs, they enable other code to store "freezed"
/// programs and to remain in control about when and for how long code runs.
pub struct Vm<B: Borrow<ByteCode>, T: Tracer> {
    // For type-safety, the VM has an API that takes ownership of the VM and
    // returns a new VM. If the VM is big, this causes lots of memcopies of
    // stack memory. So, we instead only store a pointer to the actual VM state.
    inner: Box<VmInner<B, T>>,
}

struct VmInner<B: Borrow<ByteCode>, T: Tracer> {
    byte_code: B,
    state: MachineState,
    tracer: T,
    /// When running a program normally, we first run the module which then
    /// returns the main function. To simplify this for VM users, we provide
    /// [`Vm::for_main_function`] which does both.
    ///
    /// This value is set in the above case while running the module itself, and
    /// is [`None`] in the second phase or if just running a module or function
    /// on its own.
    environment_for_main_function: Option<Struct>,
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

impl<B, T> Vm<B, T>
where
    B: Borrow<ByteCode>,
    T: Tracer,
{
    /// Run the module and then run the returned main function accepting a
    /// single parameter, the environment.
    ///
    /// This only supports byte code compiled for
    /// [`ExecutionTarget::MainFunction`].
    pub fn for_main_function(
        byte_code: B,
        heap: &mut Heap,
        environment: Struct,
        tracer: T,
    ) -> Self {
        let mut vm = Self::for_module(byte_code, heap, tracer);
        vm.inner.environment_for_main_function = Some(environment);
        vm
    }
    pub fn for_function(
        byte_code: B,
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

        let inner = Box::new(VmInner {
            byte_code,
            state,
            tracer,
            environment_for_main_function: None,
        });
        Self { inner }
    }
    pub fn for_module(byte_code: B, heap: &mut Heap, tracer: T) -> Self {
        let actual_byte_code = byte_code.borrow();
        let function = actual_byte_code.module_function;
        let responsible = actual_byte_code.responsible_module;
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
        Self::for_function(byte_code, heap, function, &[], responsible, tracer)
    }

    #[must_use]
    pub fn byte_code(&self) -> &B {
        &self.inner.byte_code
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
pub struct VmHandleCall<B: Borrow<ByteCode>, T: Tracer> {
    #[deref]
    pub call: CallHandle,
    vm: Vm<B, T>,
}
#[must_use]
pub struct VmFinished<T: Tracer> {
    pub tracer: T,
    pub result: Result<InlineObject, Panic>,
}

#[must_use]
pub enum StateAfterRun<B: Borrow<ByteCode>, T: Tracer> {
    Running(Vm<B, T>),
    CallingHandle(VmHandleCall<B, T>),
    Finished(VmFinished<T>),
}

impl<B, T> VmHandleCall<B, T>
where
    B: Borrow<ByteCode>,
    T: Tracer,
{
    pub fn complete(mut self, heap: &mut Heap, return_value: impl Into<InlineObject>) -> Vm<B, T> {
        self.handle.drop(heap);
        for argument in &self.call.arguments {
            argument.drop(heap);
        }

        self.vm.inner.state.data_stack.push(return_value.into());
        self.vm
    }
}

impl<B, T> Vm<B, T>
where
    B: Borrow<ByteCode>,
    T: Tracer,
{
    /// Runs one instruction in the VM and returns its new state.
    pub fn run(mut self, heap: &mut Heap) -> StateAfterRun<B, T> {
        let Some(current_instruction) = self.inner.state.next_instruction else {
            let return_value = self.inner.state.data_stack.pop().unwrap();
            self.inner.tracer.call_ended(heap, return_value);

            if let Some(environment) = self.inner.environment_for_main_function {
                // We just ran the whole module which returned the main
                // function. Now execute this main function using the
                // environment we received earlier.
                let responsible = HirId::create(heap, true, hir::Id::user());
                let new_vm = Self::for_function(
                    self.inner.byte_code,
                    heap,
                    return_value.try_into().unwrap(),
                    &[environment.into()],
                    responsible,
                    self.inner.tracer,
                );
                return StateAfterRun::Running(new_vm);
            }

            return StateAfterRun::Finished(VmFinished {
                tracer: self.inner.tracer,
                result: Ok(return_value),
            });
        };

        let instruction = self
            .inner
            .byte_code
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
    pub fn run_n(mut self, heap: &mut Heap, max_instructions: usize) -> StateAfterRun<B, T> {
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
pub enum StateAfterRunForever<B: Borrow<ByteCode>, T: Tracer> {
    CallingHandle(VmHandleCall<B, T>),
    Finished(VmFinished<T>),
}

impl<B, T> Vm<B, T>
where
    B: Borrow<ByteCode>,
    T: Tracer,
{
    /// Runs the VM until a handle call is performed, the VM returns, or it
    /// panics.
    pub fn run_forever(mut self, heap: &mut Heap) -> StateAfterRunForever<B, T> {
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
