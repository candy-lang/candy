use crate::{
    heap::{Data, Handle, Heap, InlineObject, List, Struct, Tag, Text},
    lir::Lir,
    tracer::Tracer,
    vm::VmHandleCall,
    StateAfterRun, StateAfterRunForever, Vm, VmFinished,
};
use itertools::Itertools;
use std::{
    borrow::Borrow,
    io::{self, BufRead},
};
use tracing::info;

pub trait Environment {
    fn handle<L: Borrow<Lir>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<L, T>,
    ) -> Vm<L, T>;
}

pub struct EmptyEnvironment;
impl Environment for EmptyEnvironment {
    fn handle<L: Borrow<Lir>, T: Tracer>(
        &mut self,
        _heap: &mut Heap,
        _call: VmHandleCall<L, T>,
    ) -> Vm<L, T> {
        panic!("A handle was called.")
    }
}
impl<L: Borrow<Lir>, T: Tracer> Vm<L, T> {
    pub fn run_without_handles(self, heap: &mut Heap) -> StateAfterRunWithoutHandles<L, T> {
        self.run_with_environment(heap, &mut EmptyEnvironment)
    }
    pub fn run_n_without_handles(
        self,
        heap: &mut Heap,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<L, T> {
        self.run_n_with_environment(heap, &mut EmptyEnvironment, max_instructions)
    }
    pub fn run_forever_without_handles(self, heap: &mut Heap) -> VmFinished<T> {
        self.run_forever_with_environment(heap, &mut EmptyEnvironment)
    }
}

pub struct DefaultEnvironment {
    stdin_handle: Handle,
    stdout_handle: Handle,
}
impl DefaultEnvironment {
    pub fn new(heap: &mut Heap, args: &[String]) -> (InlineObject, Self) {
        let arguments = args
            .iter()
            .map(|it| Text::create(heap, true, it).into())
            .collect_vec();
        let arguments = List::create(heap, true, arguments.as_slice());
        let stdin_handle = Handle::new(heap, 0);
        let stdout_handle = Handle::new(heap, 1);
        let environment_object = Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (heap.default_symbols().arguments, arguments.into()),
                (heap.default_symbols().stdout, **stdout_handle),
                (heap.default_symbols().stdin, **stdin_handle),
            ],
        );
        let environment = Self {
            stdin_handle,
            stdout_handle,
        };
        (environment_object.into(), environment)
    }
}
impl Environment for DefaultEnvironment {
    fn handle<L: Borrow<Lir>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<L, T>,
    ) -> Vm<L, T> {
        if call.handle == self.stdin_handle {
            let input = {
                let stdin = io::stdin();
                stdin.lock().lines().next().unwrap().unwrap()
            };
            let text = Text::create(heap, true, &input);
            call.complete(heap, text)
        } else if call.handle == self.stdout_handle {
            let message = call.arguments[0];

            if let Data::Text(text) = message.into() {
                println!("{}", text.get());
            } else {
                info!("Non-text value sent to stdout: {message:?}");
            }
            let nothing = Tag::create_nothing(heap);
            call.complete(heap, nothing)
        } else {
            unreachable!()
        }
    }
}

#[must_use]
pub enum StateAfterRunWithoutHandles<L: Borrow<Lir>, T: Tracer> {
    Running(Vm<L, T>),
    Finished(VmFinished<T>),
}
impl<L: Borrow<Lir>, T: Tracer> Vm<L, T> {
    pub fn run_with_environment(
        self,
        heap: &mut Heap,
        environment: &mut impl Environment,
    ) -> StateAfterRunWithoutHandles<L, T> {
        match self.run(heap) {
            StateAfterRun::Running(vm) => StateAfterRunWithoutHandles::Running(vm),
            StateAfterRun::CallingHandle(call) => {
                StateAfterRunWithoutHandles::Running(environment.handle(heap, call))
            }
            StateAfterRun::Finished(finished) => StateAfterRunWithoutHandles::Finished(finished),
        }
    }

    pub fn run_n_with_environment(
        mut self,
        heap: &mut Heap,
        environment: &mut impl Environment,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<L, T> {
        for _ in 0..max_instructions {
            match self.run_with_environment(heap, environment) {
                StateAfterRunWithoutHandles::Running(vm) => self = vm,
                finished @ StateAfterRunWithoutHandles::Finished(_) => return finished,
            }
        }
        StateAfterRunWithoutHandles::Running(self)
    }

    pub fn run_forever_with_environment(
        mut self,
        heap: &mut Heap,
        environment: &mut impl Environment,
    ) -> VmFinished<T> {
        loop {
            match self.run_forever(heap) {
                StateAfterRunForever::CallingHandle(call) => self = environment.handle(heap, call),
                StateAfterRunForever::Finished(finished) => return finished,
            }
        }
    }
}