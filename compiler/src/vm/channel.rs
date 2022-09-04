use itertools::Itertools;
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
pub struct Channel {
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

impl Channel {
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

    pub fn send(&mut self, packet: Packet) -> bool {
        if self.is_full() {
            return false;
        }
        self.packets.push_back(packet);
        return true;
    }

    pub fn receive(&mut self) -> Option<Packet> {
        self.packets.pop_front()
    }
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "<empty>")
        } else {
            write!(f, "{}", self.packets.iter().map(|packet| format!("{:?}", packet)).join(", "))
        }
    }
}
impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value.format(&self.heap))
    }
}
