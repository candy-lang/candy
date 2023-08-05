use crate::{
    fiber::FiberId,
    heap::{Heap, HeapObject, InlineObject},
    vm::OperationId,
};
use candy_frontend::impl_countable_id;
use itertools::Itertools;
use std::{
    collections::VecDeque,
    fmt::{self, Debug, Formatter},
};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChannelId(usize);

impl_countable_id!(ChannelId);
impl Debug for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "channel_{:x}", self.0)
    }
}

/// A conveyer belt or pipe that flows between send and receive ports in the
/// program. Using send ports, you can put packets into a channel. Using receive
/// ports, you can get packets out again.
///
/// Channels always have a maximum capacity of packets that they can hold
/// simultaneously – you can set it to something large, but having no limit
/// enables buggy code that leaks memory.
#[derive(Clone)]
struct ChannelBuf {
    pub capacity: Capacity,
    packets: VecDeque<Packet>,
}

pub type Capacity = usize;

/// A self-contained value that is sent over a channel.
pub struct Packet {
    pub heap: Heap,
    pub object: InlineObject,
}
impl Clone for Packet {
    fn clone(&self) -> Self {
        let (heap, mapping) = self.heap.clone();
        let object = match HeapObject::try_from(self.object) {
            Ok(heap_object) => mapping[&heap_object].into(),
            Err(_) => self.object,
        };
        Self { heap, object }
    }
}
impl From<InlineObject> for Packet {
    fn from(object: InlineObject) -> Self {
        let mut heap = Heap::default();
        let object = object.clone_to_heap(&mut heap);
        Self { heap, object }
    }
}

impl ChannelBuf {
    fn new(capacity: Capacity) -> Self {
        Self {
            capacity,
            packets: VecDeque::with_capacity(capacity),
        }
    }

    fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
    fn is_full(&self) -> bool {
        self.packets.len() == self.capacity
    }

    fn send(&mut self, packet: Packet) {
        if self.is_full() {
            panic!("Tried to send on a channel that is full.");
        }
        self.packets.push_back(packet);
    }

    fn receive(&mut self) -> Packet {
        self.packets
            .pop_front()
            .expect("Tried to receive from a channel that is empty.")
    }
}

/// A wrapper around `ChannelBuf` that also stores pending operations and
/// completes them lazily.
#[derive(Clone)]
pub struct Channel {
    buffer: ChannelBuf,
    pending_sends: VecDeque<(Performer, Packet)>,
    pending_receives: VecDeque<Performer>,
}

#[derive(Clone)]
pub enum Performer {
    Fiber(FiberId),
    Nursery,
    External(OperationId),
}

pub trait Completer {
    fn complete_send(&mut self, performer: Performer);
    fn complete_receive(&mut self, performer: Performer, received: Packet);
}

impl Channel {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: ChannelBuf::new(capacity),
            pending_sends: Default::default(),
            pending_receives: Default::default(),
        }
    }

    pub fn send(&mut self, completer: &mut dyn Completer, performer: Performer, packet: Packet) {
        self.pending_sends.push_back((performer, packet));
        self.work_on_pending_operations(completer);
    }

    pub fn receive(&mut self, completer: &mut dyn Completer, performer: Performer) {
        self.pending_receives.push_back(performer);
        self.work_on_pending_operations(completer);
    }

    fn work_on_pending_operations(&mut self, completer: &mut dyn Completer) {
        if self.buffer.capacity == 0 {
            while !self.pending_sends.is_empty() && !self.pending_receives.is_empty() {
                let (sender, packet) = self.pending_sends.pop_front().unwrap();
                let receiver = self.pending_receives.pop_front().unwrap();
                completer.complete_send(sender);
                completer.complete_receive(receiver, packet);
            }
        } else {
            loop {
                let mut did_perform_operation = false;

                if !self.buffer.is_full() && let Some((performer, packet)) = self.pending_sends.pop_front() {
                            self.buffer.send(packet);
                            completer.complete_send(performer);
                            did_perform_operation = true;
                        }

                if !self.buffer.is_empty() && let Some(performer) = self.pending_receives.pop_front() {
                            let packet = self.buffer.receive();
                            completer.complete_receive(performer, packet);
                            did_perform_operation = true;
                        }

                if !did_perform_operation {
                    break;
                }
            }
        }
    }
}

impl Debug for ChannelBuf {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_list().entries(self.packets.iter()).finish()
    }
}
impl Debug for Packet {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.object)
    }
}
impl Debug for Channel {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Channel")
            .field("buffer", &self.buffer)
            .field(
                "operations",
                &self
                    .pending_sends
                    .iter()
                    .map(|(performer, packet)| {
                        format!(
                            "{}sending {:?}",
                            if let Performer::Fiber(fiber) = performer {
                                format!("{:?} ", fiber)
                            } else {
                                "".to_string()
                            },
                            packet
                        )
                    })
                    .chain(self.pending_receives.iter().map(|performer| {
                        format!(
                            "{}receiving",
                            if let Performer::Fiber(fiber) = performer {
                                format!("{:?} ", fiber)
                            } else {
                                "".to_string()
                            },
                        )
                    }))
                    .collect_vec(),
            )
            .finish()
    }
}
