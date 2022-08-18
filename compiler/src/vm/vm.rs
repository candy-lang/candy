use super::{
    channel::{Channel, Packet},
    fiber::Fiber,
    heap::ChannelId,
    tracer::Tracer,
    use_provider::{DbUseProvider, UseProvider},
    Closure, Heap, Pointer, TearDownResult,
};
use crate::{database::Database, vm::fiber};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info, trace};

/// A VM is a Candy program that thinks it's currently running. Because VMs are
/// first-class Rust structs, they enable other code to store "freezed" programs
/// and to remain in control about when and for how long they run.
#[derive(Clone)]
pub struct Vm {
    status: Status,
    pub fiber: Fiber,

    // Channel functionality. VMs communicate with the outer world using
    // channels. Each channel is identified using an ID that is valid inside
    // this particular VM. Channels created by the program are managed ("owned")
    // by the VM itself. For channels owned by the outside world (such as those
    // referenced in the environment argument), the VM maintains a mapping
    // between internal and external IDs.
    pub internal_channels: HashMap<ChannelId, Channel>,
    pub next_internal_channel_id: ChannelId,
    pub external_to_internal_channels: HashMap<ChannelId, ChannelId>,
    pub internal_to_external_channels: HashMap<ChannelId, ChannelId>,
    pub channel_operations: HashMap<ChannelId, VecDeque<ChannelOperation>>,
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    Done,
    Panicked { reason: String },
}

#[derive(Clone)]
pub enum ChannelOperation {
    Send { packet: Packet },
    Receive,
}

impl Vm {
    fn new_with_fiber(fiber: Fiber) -> Self {
        Self {
            status: Status::Running,
            fiber,
            internal_channels: Default::default(),
            next_internal_channel_id: 0,
            external_to_internal_channels: Default::default(),
            internal_to_external_channels: Default::default(),
            channel_operations: Default::default(),
        }
    }
    pub fn new_for_running_closure<U: UseProvider>(
        heap: Heap,
        use_provider: &U,
        closure: Pointer,
        arguments: &[Pointer],
    ) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_closure(
            heap,
            use_provider,
            closure,
            arguments,
        ))
    }
    pub fn new_for_running_module_closure<U: UseProvider>(
        use_provider: &U,
        closure: Closure,
    ) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_module_closure(use_provider, closure))
    }
    pub fn tear_down(self) -> TearDownResult {
        self.fiber.tear_down()
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }
    pub fn cloned_tracer(&self) -> Tracer {
        self.fiber.tracer.clone()
    }
    pub fn num_instructions_executed(&self) -> usize {
        self.fiber.num_instructions_executed
    }

    pub fn run<U: UseProvider>(&mut self, use_provider: &U, mut num_instructions: usize) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        loop {
            debug!("Running fiber (status = {:?}).", self.fiber.status);
            self.fiber.run(use_provider, num_instructions);
            match self.fiber.status() {
                fiber::Status::Running => {}
                fiber::Status::CreatingChannel { capacity } => {
                    let id = self.next_internal_channel_id;
                    self.next_internal_channel_id += 1;
                    self.internal_channels.insert(id, Channel::new(capacity));
                    self.fiber.complete_channel_create(id);
                }
                fiber::Status::Sending { channel, packet } => {
                    if let Some(channel) = self.internal_channels.get_mut(&channel) {
                        // Internal channel.
                        if channel.send(packet) {
                            self.fiber.complete_send();
                        } else {
                            panic!("Sent to internal full channel. Deadlock.");
                        }
                    } else {
                        // External channel.
                        todo!()
                    }
                }
                fiber::Status::Receiving { channel } => {
                    if let Some(channel) = self.internal_channels.get_mut(&channel) {
                        // Internal channel.
                        if let Some(packet) = channel.receive() {
                            self.fiber.complete_receive(packet);
                        } else {
                            panic!("Tried to receive from internal empty channel. Deadlock.");
                        }
                    } else {
                        // External channel.
                        todo!()
                    }
                }
                fiber::Status::Done => {
                    self.status = Status::Done;
                    break;
                }
                fiber::Status::Panicked { reason } => {
                    self.status = Status::Panicked { reason };
                    break;
                }
            }
        }
    }
    pub fn run_synchronously_until_completion(mut self, db: &Database) -> TearDownResult {
        let use_provider = DbUseProvider { db };
        loop {
            self.run(&use_provider, 100000);
            match self.status() {
                Status::Running => info!("Code is still running."),
                _ => return self.tear_down(),
            }
        }
    }
}
