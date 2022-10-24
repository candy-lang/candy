use super::{ids::OperationId, FiberId, Heap, Pointer};
use itertools::Itertools;
use std::{collections::VecDeque, fmt};

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
#[derive(Clone)]
pub struct Packet {
    pub heap: Heap,
    pub value: Pointer,
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
impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
