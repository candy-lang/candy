use crate::{
    byte_code::ByteCode,
    heap::{Data, Handle, Heap, InlineObject, Int, List, Struct, Tag, Text},
    tracer::Tracer,
    vm::VmHandleCall,
    StateAfterRun, StateAfterRunForever, Vm, VmFinished,
};
use itertools::Itertools;
use std::{
    borrow::Borrow,
    io::{self, BufRead},
    time::SystemTime,
};
use tracing::info;

pub trait Environment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<B, T>,
    ) -> Vm<B, T>;
}

pub struct EmptyEnvironment;
impl Environment for EmptyEnvironment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        _heap: &mut Heap,
        _call: VmHandleCall<B, T>,
    ) -> Vm<B, T> {
        panic!("A handle was called.")
    }
}
impl<B: Borrow<ByteCode>, T: Tracer> Vm<B, T> {
    pub fn run_without_handles(self, heap: &mut Heap) -> StateAfterRunWithoutHandles<B, T> {
        self.run_with_environment(heap, &mut EmptyEnvironment)
    }
    pub fn run_n_without_handles(
        self,
        heap: &mut Heap,
        max_instructions: usize,
    ) -> StateAfterRunWithoutHandles<B, T> {
        self.run_n_with_environment(heap, &mut EmptyEnvironment, max_instructions)
    }
    pub fn run_forever_without_handles(self, heap: &mut Heap) -> VmFinished<T> {
        self.run_forever_with_environment(heap, &mut EmptyEnvironment)
    }
}

pub struct DefaultEnvironment {
    // Sorted alphabetically
    get_random_bytes_handle: Handle,
    stdin_handle: Handle,
    stdout_handle: Handle,
    system_clock_handle: Handle,
}
impl DefaultEnvironment {
    pub fn new(heap: &mut Heap, args: &[String]) -> (InlineObject, Self) {
        let arguments = args
            .iter()
            .map(|it| Text::create(heap, true, it).into())
            .collect_vec();
        let arguments = List::create(heap, true, arguments.as_slice());
        let get_random_bytes_handle = Handle::new(heap, 1);
        let stdin_handle = Handle::new(heap, 0);
        let stdout_handle = Handle::new(heap, 1);
        let system_clock_handle = Handle::new(heap, 0);
        let environment_object = Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (heap.default_symbols().arguments, arguments.into()),
                (
                    heap.default_symbols().get_random_bytes,
                    **get_random_bytes_handle,
                ),
                (heap.default_symbols().stdin, **stdin_handle),
                (heap.default_symbols().stdout, **stdout_handle),
                (heap.default_symbols().system_clock, **system_clock_handle),
            ],
        );
        let environment = Self {
            get_random_bytes_handle,
            stdin_handle,
            stdout_handle,
            system_clock_handle,
        };
        (environment_object.into(), environment)
    }
}
impl Environment for DefaultEnvironment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<B, T>,
    ) -> Vm<B, T> {
        if call.handle == self.get_random_bytes_handle {
            let [length] = call.arguments.as_slice() else {
                unreachable!()
            };

            let Data::Int(length) = (*length).into() else {
                // TODO: Panic
                let message = Text::create(
                    heap,
                    true,
                    "Handle `getRandomBytes` was called with a non-integer.",
                );
                let result = Tag::create_result(heap, true, Err(message.into()));
                return call.complete(heap, result);
            };
            let Some(length) = length.try_get::<usize>() else {
                // TODO: Panic
                let message = Text::create(
                    heap,
                    true,
                    "Handle `getRandomBytes` was called with a length that doesn't fit in usize.",
                );
                let result = Tag::create_result(heap, true, Err(message.into()));
                return call.complete(heap, result);
            };

            let mut bytes = vec![0u8; length];
            if let Err(error) = getrandom::getrandom(&mut bytes) {
                let message = Text::create(heap, true, &error.to_string());
                let result = Tag::create_result(heap, true, Err(message.into()));
                return call.complete(heap, result);
            }

            let bytes = bytes
                .into_iter()
                .map(|it| Int::create(heap, true, it).into())
                .collect_vec();
            let bytes = List::create(heap, true, bytes.as_slice());
            let result = Tag::create_result(heap, true, Ok(bytes.into()));
            call.complete(heap, result)
        } else if call.handle == self.stdin_handle {
            let [] = call.arguments.as_slice() else {
                unreachable!()
            };
            let input = {
                let stdin = io::stdin();
                stdin.lock().lines().next().unwrap().unwrap()
            };
            let text = Text::create(heap, true, &input);
            call.complete(heap, text)
        } else if call.handle == self.stdout_handle {
            let [message] = call.arguments.as_slice() else {
                unreachable!()
            };

            if let Data::Text(text) = (*message).into() {
                println!("{}", text.get());
            } else {
                info!("Non-text value sent to stdout: {message:?}");
            }

            let nothing = Tag::create_nothing(heap);
            call.complete(heap, nothing)
        } else if call.handle == self.system_clock_handle {
            let [] = call.arguments.as_slice() else {
                unreachable!()
            };

            let now = SystemTime::now();
            let since_unix_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            let nanoseconds = Int::create(heap, true, since_unix_epoch.as_nanos());
            call.complete(heap, nanoseconds)
        } else {
            unreachable!()
        }
    }
}

#[must_use]
pub enum StateAfterRunWithoutHandles<B: Borrow<ByteCode>, T: Tracer> {
    Running(Vm<B, T>),
    Finished(VmFinished<T>),
}
impl<B: Borrow<ByteCode>, T: Tracer> Vm<B, T> {
    pub fn run_with_environment(
        self,
        heap: &mut Heap,
        environment: &mut impl Environment,
    ) -> StateAfterRunWithoutHandles<B, T> {
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
    ) -> StateAfterRunWithoutHandles<B, T> {
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
