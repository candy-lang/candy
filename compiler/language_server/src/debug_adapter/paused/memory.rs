use super::PausedState;
use base64::Engine;
use candy_vm::heap::{HeapData, HeapObject, HeapObjectTrait, InlineObject, ObjectInHeap};
use dap::{requests::ReadMemoryArguments, responses::ReadMemoryResponse};
use extension_trait::extension_trait;
use std::{
    borrow::Cow,
    mem::size_of,
    num::NonZeroUsize,
    ops::Range,
    ptr::{slice_from_raw_parts, NonNull},
};

impl PausedState {
    #[allow(unused_parens)]
    pub fn read_memory(
        &mut self,
        args: &ReadMemoryArguments,
    ) -> Result<ReadMemoryResponse, &'static str> {
        let reference = MemoryReference::from_dap(&args.memory_reference)?;
        let (base_offset, actual_range, data) = match reference {
            MemoryReference::Inline { value } => {
                let bytes = value.raw_word().get().to_ne_bytes();
                let range = 0..bytes.len();
                (0, range, Cow::Owned(bytes.to_vec()))
            }
            MemoryReference::Heap { address } => {
                let object = HeapObject::new(NonNull::new(address.get() as *mut u64).unwrap());
                if !self.heap_ref().objects().contains(&ObjectInHeap(object)) {
                    return Err("memory-reference-invalid");
                }
                let range = HeapData::from(object).address_range();
                let range = range.start.get()..range.end.get();

                let data = slice_from_raw_parts(range.start as *const u8, range.len());
                let data = unsafe { &*data };

                (address.get(), range, Cow::Borrowed(data))
            }
        };

        let requested_start = base_offset + args.offset.unwrap_or_default();
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
pub enum MemoryReference {
    Inline { value: InlineObject },
    Heap { address: NonZeroUsize },
}
impl MemoryReference {
    pub fn new(value: InlineObject) -> Self {
        HeapObject::try_from(value).map_or_else(|_| Self::Inline { value }, Self::heap)
    }
    pub fn heap(object: HeapObject) -> Self {
        Self::Heap {
            address: object.address().addr(),
        }
    }

    pub fn from_dap(value: &str) -> Result<Self, &'static str> {
        let mut parts = value.split('-');

        match parts.next().ok_or("heap-inline-disambiguator-missing")? {
            "heap" => {
                let address = parts.next().ok_or("memory-address-missing")?;
                let address = usize::from_str_radix(address, 16)
                    .ok()
                    .and_then(|it| it.try_into().ok())
                    .ok_or("memory-address-invalid")?;

                Ok(Self::Heap { address })
            }
            "inline" => {
                let value = parts.next().ok_or("value-missing")?;
                let value = u64::from_str_radix(value, 16)
                    .ok()
                    .and_then(|it| it.try_into().ok())
                    .map(InlineObject::new)
                    .ok_or("memory-value-invalid")?;

                Ok(Self::Inline { value })
            }
            _ => Err("heap-inline-disambiguator-invalid"),
        }
    }
    pub fn to_dap(self) -> String {
        match self {
            Self::Inline { value } => format!(
                "inline-{:0width$X}",
                value.raw_word(),
                width = 2 * size_of::<usize>(),
            ),
            Self::Heap { address } => {
                format!("heap-{address:016X}")
            }
        }
    }
}

#[extension_trait]
impl<T: Copy + Ord> RangeExtension<T> for Range<T> {
    fn intersection(&self, other: &Self) -> Self {
        self.start.max(other.start)..self.end.min(other.end)
    }
}
