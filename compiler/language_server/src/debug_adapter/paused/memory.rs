use super::PausedState;
use base64::Engine;
use candy_frontend::id::CountableId;
use candy_vm::{
    fiber::FiberId,
    heap::{Heap, HeapData, HeapObject, HeapObjectTrait, InlineObject, ObjectInHeap},
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

        let start_address = reference
            .address
            .get()
            .saturating_sub(args.offset.unwrap_or_default());
        let end_address = start_address + args.count;
        let range = fiber
            .heap
            .first_contiguous_range_in(start_address..end_address);
        let Some(range) = range else {
            return Ok(ReadMemoryResponse {
                address: format_address(start_address),
                unreadable_bytes: Some(args.count),
                data: None,
            });
        };

        let data = slice_from_raw_parts(
            range.start.get() as *const u8,
            range.end.get().min(end_address) - range.start.get(),
        );
        let data = unsafe { &*data };
        let data = base64::engine::general_purpose::STANDARD.encode(data);

        let next_address = fiber
            .heap
            .first_object_range_in(range.end.get()..end_address)
            .map(|it| it.start);

        Ok(ReadMemoryResponse {
            address: format_address(range.start.get()),
            unreadable_bytes: next_address.map(|it| it.get() - range.end.get()),
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
    // TODO: View memory across all fibers
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
impl HeapExtension for Heap {
    fn first_contiguous_range_in(
        &self,
        address_range: Range<usize>,
    ) -> Option<Range<NonZeroUsize>> {
        let mut range = self.first_object_range_in(address_range)?;
        loop {
            if range.end.get() % HeapObject::WORD_SIZE != 0 {
                break;
            }

            let next_object = HeapObject::new(NonNull::new(range.end.get() as *mut u64).unwrap());
            if !self.objects().contains(&ObjectInHeap(next_object)) {
                break;
            }

            range.end = HeapData::from(next_object).address_range().end;
        }
        Some(range)
    }
    fn first_object_range_in(&self, address_range: Range<usize>) -> Option<Range<NonZeroUsize>> {
        self.objects()
            .iter()
            .map(|&it| HeapData::from(*it).address_range())
            .filter(|it| (it.start.get()..it.end.get()).overlaps(&address_range))
            .min_by_key(|it| it.start)
    }
}

#[extension_trait]
impl<T: Ord> RangeExtension<T> for Range<T> {
    fn overlaps(&self, other: &Range<T>) -> bool {
        self.start < other.end && other.start < self.end
    }
}
