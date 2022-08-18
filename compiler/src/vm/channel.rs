use super::{Heap, Pointer};
use std::collections::VecDeque;

/// A conveyer belt or pipe that flows between send and receive ports in the
/// program. Using send ports, you can put packets into a channel. Using receive
/// ports, you can get packets out again.
///
/// Channels always have a maximum capacity of packets that they can hold
/// simultaneously – you can set it to something large, but having no capacity
/// enables buggy code that leaks memory.
#[derive(Clone)]
pub struct Channel {
    pub capacity: usize,
    packets: VecDeque<Packet>,
}

/// A self-contained value that is sent over a channel.
#[derive(Clone)]
pub struct Packet {
    heap: Heap,
    value: Pointer,
}

impl Channel {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            packets: Default::default(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.packets.len() == self.capacity
    }

    pub fn send(&mut self, packet: Packet) {
        assert!(!self.is_full());
        self.packets.push_back(packet);
    }

    pub fn receive(&mut self) -> Option<Packet> {
        self.packets.pop_front()
    }
}
