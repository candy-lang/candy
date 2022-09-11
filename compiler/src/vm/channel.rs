use super::{Heap, Pointer};
use std::{collections::VecDeque, fmt};

/// A conveyer belt or pipe that flows between send and receive ports in the
/// program. Using send ports, you can put packets into a channel. Using receive
/// ports, you can get packets out again.
///
/// Channels always have a maximum capacity of packets that they can hold
/// simultaneously – you can set it to something large, but having no capacity
/// enables buggy code that leaks memory.
#[derive(Clone)]
pub struct ChannelBuf {
    pub capacity: Capacity,
    packets: VecDeque<Packet>,
}

pub type Capacity = usize;

/// A self-contained value that is sent over a channel.
#[derive(Clone)]
pub struct Packet {
    pub heap: Heap,
    pub value: Pointer,
}

impl ChannelBuf {
    pub fn new(capacity: Capacity) -> Self {
        Self {
            capacity,
            packets: VecDeque::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.packets.len() == self.capacity
    }

    pub fn send(&mut self, packet: Packet) {
        if self.is_full() {
            panic!("Tried to send on channel that is full.");
        }
        self.packets.push_back(packet);
    }

    pub fn receive(&mut self) -> Packet {
        self.packets
            .pop_front()
            .expect("Tried to receive from channel that is empty.")
    }
}

impl fmt::Debug for ChannelBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.packets.iter()).finish()
    }
}
impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value.format(&self.heap))
    }
}
