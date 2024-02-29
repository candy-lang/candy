use candy_vm::{
    byte_code::ByteCode,
    environment::Environment,
    heap::{Data, Handle, Heap, InlineObject, Int, List, Struct, Tag, Text},
    tracer::Tracer,
    Vm, VmHandleCall,
};
use itertools::Itertools;
use std::{borrow::Borrow, iter};

pub struct BenchmarkingEnvironment {
    get_random_bytes_handle: Handle,
    stdout_handle: Handle,
}
impl BenchmarkingEnvironment {
    pub fn new(heap: &mut Heap, args: &[&str]) -> (Struct, Self) {
        let arguments = args
            .iter()
            .map(|it| Text::create(heap, true, it).into())
            .collect_vec();
        let arguments = List::create(heap, true, arguments.as_slice());

        let get_random_bytes_handle = Handle::new(heap, 1);
        let stdout_handle = Handle::new(heap, 1);

        let environment_object = Struct::create_with_symbol_keys(
            heap,
            true,
            [
                (heap.default_symbols().arguments, arguments.into()),
                (
                    heap.default_symbols().get_random_bytes,
                    **get_random_bytes_handle,
                ),
                (heap.default_symbols().stdout, **stdout_handle),
            ],
        );
        let environment = Self {
            get_random_bytes_handle,
            stdout_handle,
        };
        (environment_object, environment)
    }
}
impl Environment for BenchmarkingEnvironment {
    fn handle<B: Borrow<ByteCode>, T: Tracer>(
        &mut self,
        heap: &mut Heap,
        call: VmHandleCall<B, T>,
    ) -> Vm<B, T> {
        let result: InlineObject = if call.handle == self.get_random_bytes_handle {
            let [length] = call.arguments.as_slice() else {
                unreachable!()
            };
            let Data::Int(length) = (*length).into() else {
                panic!("Handle `getRandomBytes` was called with a non-integer length.")
            };
            let Some(length) = length.try_get::<usize>() else {
                panic!("Handle `getRandomBytes` was called with a length that doesn't fit Rust's `usize`.")
            };

            let bytes = iter::repeat(42u8)
                .take(length)
                .map(|it| Int::create(heap, true, it).into())
                .collect_vec();
            let bytes = List::create(heap, true, bytes.as_slice());
            Tag::create_result(heap, true, Ok(bytes.into())).into()
        } else if call.handle == self.stdout_handle {
            // We don't use the output while benchmarking, so we can just ignore
            // this call.
            Tag::create_nothing(heap).into()
        } else {
            panic!("A handle was called that doesn't exist: {:?}", call.handle)
        };
        call.complete(heap, result)
    }
}
