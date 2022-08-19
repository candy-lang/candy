use super::{
    channel::{Channel, Packet},
    fiber::Fiber,
    heap::ChannelId,
    tracer::Tracer,
    use_provider::{DbUseProvider, UseProvider},
    Closure, Heap, Pointer, TearDownResult,
};
use crate::{database::Database, vm::fiber};
use itertools::Itertools;
use rand::{seq::IteratorRandom, thread_rng};
use std::{
    collections::{HashMap, VecDeque},
    mem,
};
use tracing::{debug, error, info, trace, warn};

/// A VM is a Candy program that thinks it's currently running. Because VMs are
/// first-class Rust structs, they enable other code to store "freezed" programs
/// and to remain in control about when and for how long they run.
#[derive(Clone)]
pub struct Vm {
    status: Status,
    state: Option<State>, // Only `None` temporarily during state transitions.

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
    WaitingForOperations,
    Done,
    Panicked { reason: String },
}

#[derive(Clone)]
enum State {
    SingleFiber(Fiber),
    ParallelSection {
        /// The main fiber of this VM. Should have Status::InParallelSection.
        paused_main_fiber: Fiber,

        /// The channel that you can send spawn commands to.
        nursery: ChannelId,

        /// The VM for the closure with which `core.parallel` was called.
        parallel_body: Box<Vm>,

        /// Spawned child VMs. For each VM, there also exists a channel that
        /// will contain the result of the VM (once it's done or panicked). This
        /// channel is directly returned by the `core.async` function.
        /// Here, we save the ID of the channel where the result of the VM will
        /// be sent.
        spawned_children: Vec<(ChannelId, Vm)>,
    },
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
            state: Some(State::SingleFiber(fiber)),
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
        match self.into_state() {
            State::SingleFiber(fiber) => fiber.tear_down(),
            State::ParallelSection { .. } => {
                panic!("Called `Vm::tear_down` while in parallel scope")
            }
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }
    pub fn fiber(&self) -> &Fiber {
        match self.state() {
            State::SingleFiber(fiber) => fiber,
            State::ParallelSection {
                paused_main_fiber, ..
            } => paused_main_fiber,
        }
    }
    pub fn cloned_tracer(&self) -> Tracer {
        self.fiber().tracer.clone()
    }
    pub fn num_instructions_executed(&self) -> usize {
        self.fiber().num_instructions_executed
    }
    fn into_state(self) -> State {
        self.state
            .expect("Tried to get VM state during state transition")
    }
    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("Tried to get VM state during state transition")
    }

    pub fn run<U: UseProvider>(&mut self, use_provider: &U, mut num_instructions: usize) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        let mut state = mem::replace(&mut self.state, None).unwrap();
        while matches!(self.status, Status::Running) && num_instructions > 0 {
            let (new_state, num_instructions_executed) =
                self.run_and_map_state(state, use_provider, num_instructions);
            state = new_state;

            if num_instructions_executed >= num_instructions {
                break;
            } else {
                num_instructions -= num_instructions_executed;
            }
        }
        self.state = Some(state);
    }
    fn run_and_map_state<U: UseProvider>(
        &mut self,
        state: State,
        use_provider: &U,
        num_instructions: usize,
    ) -> (State, usize) {
        let mut num_instructions_executed = 0;
        let new_state = 'new_state: {
            match state {
                State::SingleFiber(mut fiber) => {
                    debug!("Running fiber (status = {:?}).", fiber.status);

                    fiber.num_instructions_executed = 0;
                    fiber.run(use_provider, num_instructions);
                    num_instructions_executed = fiber.num_instructions_executed;

                    match fiber.status() {
                        fiber::Status::Running => {}
                        fiber::Status::CreatingChannel { capacity } => {
                            let id = self.generate_channel_id();
                            self.internal_channels.insert(id, Channel::new(capacity));
                            fiber.complete_channel_create(id);
                        }
                        fiber::Status::Sending { channel, packet } => {
                            if let Some(channel) = self.internal_channels.get_mut(&channel) {
                                // Internal channel.
                                if channel.send(packet) {
                                    fiber.complete_send();
                                } else {
                                    warn!("Tried to send to a full channel that is local to a fiber. This will never complete.");
                                }
                            } else {
                                // External channel.
                                let channel = self.internal_to_external_channels[&channel];
                                self.channel_operations
                                    .entry(channel)
                                    .or_default()
                                    .push_back(ChannelOperation::Send { packet });
                                self.status = Status::WaitingForOperations;
                            }
                        }
                        fiber::Status::Receiving { channel } => {
                            if let Some(channel) = self.internal_channels.get_mut(&channel) {
                                // Internal channel.
                                if let Some(packet) = channel.receive() {
                                    fiber.complete_receive(packet);
                                } else {
                                    warn!(
                                        "Tried to receive from an empty channel that is local to a fiber. This will never complete."
                                    );
                                }
                            } else {
                                // External channel.
                                let channel = self.internal_to_external_channels[&channel];
                                self.channel_operations
                                    .entry(channel)
                                    .or_default()
                                    .push_back(ChannelOperation::Receive);
                                self.status = Status::WaitingForOperations;
                            }
                        }
                        fiber::Status::InParallelScope { body } => {
                            let mut heap = Heap::default();
                            let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                            let nursery = self.generate_channel_id();
                            let nursery_send_port = heap.create_send_port(nursery);
                            break 'new_state State::ParallelSection {
                                paused_main_fiber: fiber,
                                nursery,
                                parallel_body: Box::new(Vm::new_for_running_closure(
                                    heap,
                                    use_provider,
                                    body,
                                    &[nursery_send_port],
                                )),
                                spawned_children: vec![],
                            };
                        }
                        fiber::Status::Done => {
                            self.status = Status::Done;
                        }
                        fiber::Status::Panicked { reason } => {
                            self.status = Status::Panicked { reason };
                        }
                    }
                    State::SingleFiber(fiber)
                }
                State::ParallelSection {
                    paused_main_fiber,
                    nursery,
                    mut parallel_body,
                    mut spawned_children,
                } => {
                    let (index_and_result_channel, vm) = spawned_children
                        .iter_mut()
                        .enumerate()
                        .map(|(i, (channel, vm))| (Some((i, *channel)), vm))
                        .chain([(None, &mut *parallel_body)].into_iter())
                        .filter(|(_, vm)| matches!(vm.status, Status::Running))
                        .choose(&mut rand::thread_rng())
                        .expect("Tried to run Vm, but no child can run.");

                    info!("Running child VM.");
                    vm.run(use_provider, num_instructions);

                    for (channel, operations) in &vm.channel_operations {
                        // TODO
                        warn!("Handle operations on channel {channel}")
                    }

                    // If this was a spawned channel and it ended execution, the result should be
                    // transmitted to the channel that's returned by the `core.async` call.
                    if let Some((index, result_channel)) = index_and_result_channel {
                        let packet = match vm.status() {
                            Status::Done => {
                                info!("Child done.");
                                let (_, vm) = spawned_children.remove(index);
                                let TearDownResult {
                                    heap: vm_heap,
                                    result,
                                    ..
                                } = vm.tear_down();
                                let return_value = result.unwrap();
                                let mut heap = Heap::default();
                                let return_value =
                                    vm_heap.clone_single_to_other_heap(&mut heap, return_value);
                                let value = heap.create_result(Ok(return_value));
                                Some(Packet { heap, value })
                            }
                            Status::Panicked { reason } => {
                                warn!("Child panicked with reason {reason}");
                                let mut heap = Heap::default();
                                let reason = heap.create_text(reason);
                                let value = heap.create_result(Err(reason));
                                Some(Packet { heap, value })
                            }
                            _ => None,
                        };
                        if let Some(packet) = packet {
                            self.channel_operations
                                .entry(result_channel)
                                .or_default()
                                .push_back(ChannelOperation::Send { packet })
                        }
                    }

                    // Update status and state.
                    // let all_vms = spawned_children
                    //     .iter()
                    //     .map(|(_, vm)| vm)
                    //     .chain([*parallel_body].iter())
                    //     .collect_vec();

                    // let can_something_run = all_vms
                    //     .iter()
                    //     .any(|vm| matches!(vm.status, Status::Running));
                    // if can_something_run {
                    //     self.status = Status::Running
                    // }

                    // let all_finished = all_vms
                    //     .iter()
                    //     .all(|vm| matches!(vm.status, Status::Done | Status::Panicked { .. }));
                    // if all_finished {
                    //     let TearDownResult { heap, result, .. } = parallel_body.tear_down();
                    //     match result {
                    //         Ok(return_value) => {
                    //             paused_main_fiber.complete_parallel_scope(&mut heap, return_value);
                    //             break 'new_state State::SingleFiber(paused_main_fiber);
                    //         }
                    //         Err(_) => todo!(),
                    //     }
                    // }

                    State::ParallelSection {
                        paused_main_fiber,
                        nursery,
                        parallel_body,
                        spawned_children,
                    }
                }
            }
        };
        (new_state, num_instructions_executed)
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

    fn generate_channel_id(&mut self) -> ChannelId {
        let id = self.next_internal_channel_id;
        self.next_internal_channel_id += 1;
        id
    }
}
