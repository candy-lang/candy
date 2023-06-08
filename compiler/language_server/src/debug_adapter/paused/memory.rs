use super::PausedState;
use base64::Engine;
use candy_frontend::id::CountableId;
use candy_vm::{
    fiber::FiberId,
    heap::{HeapData, HeapObject, HeapObjectTrait, InlineObject, ObjectInHeap},
};
use dap::{requests::ReadMemoryArguments, responses::ReadMemoryResponse};
use extension_trait::extension_trait;
use std::{
    num::NonZeroUsize,
    ops::Range,
    ptr::{slice_from_raw_parts, NonNull},
    str::FromStr,
};

impl PausedState {
    #[allow(unused_parens)]
    pub fn read_memory(
        &mut self,
        args: ReadMemoryArguments,
    ) -> Result<ReadMemoryResponse, &'static str> {
        let reference = MemoryReference::from_dap(args.memory_reference)?;
        let fiber = self
            .vm_state
            .vm
            .fiber(reference.fiber_id)
            .ok_or("fiber-not-found")?
            .fiber_ref();

        let object = HeapObject::new(NonNull::new(reference.address.get() as *mut u64).unwrap());
        if !fiber.heap.objects().contains(&ObjectInHeap(object)) {
            return Err("memory-reference-invalid");
        }
        let actual_range = HeapData::from(object).address_range();
        let actual_range = actual_range.start.get()..actual_range.end.get();

        let requested_start = reference.address.get() + args.offset.unwrap_or_default();
        let requested_range = requested_start..requested_start + args.count;

        let range = requested_range.intersection(&actual_range);
        if range.start > requested_start {
            return Ok(ReadMemoryResponse {
                address: format_address(requested_start),
                unreadable_bytes: if range.is_empty() {
                    None
                } else {
                    Some(range.start - requested_start)
                },
                data: None,
            });
        };

        let data = slice_from_raw_parts(range.start as *const u8, range.len());
        let data = unsafe { &*data };
        let data = base64::engine::general_purpose::STANDARD.encode(data);

        Ok(ReadMemoryResponse {
            address: format_address(range.start),
            unreadable_bytes: None,
            data: Some(data),
        })
    }
}

fn format_address(address: usize) -> String {
    format!("{:#X}", address)
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryReference {
    // TODO: Support inline values
    fiber_id: FiberId,
    address: NonZeroUsize,
}
impl MemoryReference {
    pub fn new(fiber_id: FiberId, object: HeapObject) -> Self {
        Self {
            fiber_id,
            address: object.address().addr(),
        }
    }
    pub fn maybe_new(fiber_id: FiberId, object: InlineObject) -> Option<Self> {
        HeapObject::try_from(object)
            .ok()
            .map(|it| Self::new(fiber_id, it))
    }

    pub fn from_dap(value: String) -> Result<Self, &'static str> {
        let mut parts = value.split('-');

        let fiber_id = parts.next().ok_or("fiber-id-missing")?;
        let fiber_id = usize::from_str(fiber_id).map_err(|_| "fiber-id-invalid")?;
        let fiber_id = FiberId::from_usize(fiber_id);

        let address = parts.next().ok_or("memory-address-missing")?;
        let address = usize::from_str_radix(address, 16)
            .ok()
            .and_then(|it| it.try_into().ok())
            .ok_or("memory-address-invalid")?;

        Ok(Self { fiber_id, address })
    }
    pub fn to_dap(self) -> String {
        format!("{}-{:X}", self.fiber_id.to_usize(), self.address)
    }
}

#[extension_trait]
impl<T: Copy + Ord> RangeExtension<T> for Range<T> {
    fn intersection(&self, other: &Range<T>) -> Range<T> {
        self.start.max(other.start)..self.end.min(other.end)
    }
}
