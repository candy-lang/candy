use super::{
    channel::{Channel, Packet},
    fiber::Fiber,
    heap::{ChannelId, Data, ReceivePort, SendPort},
    tracer::Tracer,
    use_provider::{DbUseProvider, UseProvider},
    Closure, Heap, Pointer, TearDownResult,
};
use crate::{database::Database, vm::fiber};
use rand::seq::IteratorRandom;
use std::{
    collections::{HashMap, VecDeque},
    mem,
};
use tracing::{debug, info, warn};

/// A fiber tree is a part of or an entire Candy program that thinks it's
/// currently running. Because fiber trees are first-class Rust structs, they
/// enable other code to store "freezed" programs and to remain in control about
/// when and for how long code runs.
///
/// While fibers are simple, pure virtual machines that manage a heap and stack,
/// fiber _trees_ encapsulate fibers and manage channels that are used by them.
///
/// ## Fibers
///
/// As the name suggests, every Candy program can be represented by a tree at
/// any point in time. In particular, you can create new nodes in the tree by
/// entering a `core.parallel` scope.
///
/// ```candy
/// core.parallel { nursery ->
///   banana = core.async { "Banana" }
///   peach = core.async { "Peach" }
/// }
/// ```
///
/// In this example, after `banana` and `peach` have been assigned, both those
/// closures run concurrently. Let's walk through what happens at each point in
/// time. Before entering the `core.parallel` scope, we only have a single fiber
/// managed by a `FiberTree`.
///
/// FiberTree
///     |
///   Fiber
///   main
/// (running)
///
/// As the program enters the `core.parallel` section, the fiber tree changes
/// its state. First, it generates a channel ID for a nursery; while a nursery
/// isn't a channel, it behaves just like one (you can send it closures). Then,
/// the fiber tree creates a send port for the nursery. Finally, it spawns a new
/// fiber tree with the body code of the parallel section, giving it a send port
/// of the nursery as an argument.
///
/// When asked to run code, the fiber tree will not run the original main fiber,
/// but the body of the parallel section instead.
///
///            FiberTree
///                |
///         +------+------+
///         |             |
///       Fiber       FiberTree
///       main            |
/// (parallel scope)    Fiber
///                     body
///                   (running)
///
/// Calls to `core.async` internally just send packets to the nursery containing
/// the closures to spawn. The fiber tree knows that the channel ID is that of
/// the nursery and instead of actually saving the packets spawns new fibers.
/// The packets also contain a send port of a channel that `core.async` creates
/// locally and that is expected to be sent the result of the running closure.
///
/// After the two calls to `core.async` finished, the tree looks like this:
///
///            FiberTree
///                |
///         +------+------+----------+----------+
///         |             |          |          |
///       Fiber       FiberTree  FiberTree  FiberTree
///       main            |          |          |
/// (parallel scope)    Fiber      Fiber      Fiber
///                     body      banana      peach
///                   (running)  (running)  (running)
///
/// Now, when the tree is asked to run code, it will run a random running fiber.
///
/// Once a spawned fiber is done, its return value is stored in the
/// corresponding channel and the fiber is deleted. Once all fibers finished
/// running, the fiber tree exits the parallel section. The `core.parallel` and
/// `core.await` calls take care of actually returning the values put into the
/// channels. If any of the children panic, the parallel section itself will
/// immediately panic as well.
///
/// The internal fiber trees can of course also start their own parallel
/// sections, resulting in a nested tree.
///
/// ## Channels
///
/// In Candy code, channels only appear via their ends, the send and receive
/// ports. Those are unlike other values. In particular, they have an identity
/// and are mutable. Operations like `channel.receive` are not pure and may
/// return different values every time. Also, operations on channels are
/// blocking.
///
/// Unlike fibers, channels don't form a tree – they can go all over the place!
/// Because you can transmit ports over channels, any two parts of a fiber tree
/// could theoretically be connected via channels.
///
/// In most programs, we expect channels to stay "relatively" local. In
/// particular, most channels don't escape the fiber tree that they are created
/// in. In order to get the most benefit out of actual paralellism, it's
/// beneficial to store channels as local as possible. For example, if two
/// completely different parts of a program use channels locally to model
/// mutable variables or some other data flow, all channel operations should be
/// local only and not need to be propagated to a central location, avoiding
/// contention. This becomes even more important when (if?) we distribute
/// programs across multiple machines; local channels shouldn't require any
/// communication whatsoever.
///
/// That's why channels are stored in the local-most subtree of the Candy
/// program that has access to corresponding ports. Fibers themselves don't
/// store channels though – the surrounding nodes of the fiber trees take care
/// of managing channels.
///
/// The identity of channels is modelled using a channel ID, which is unique
/// within a node of the fiber tree. Whenever data with ports is transmitted to
/// child or parent nodes in the tree, the channel IDs of the ports need to be
/// translated.
///
/// TODO: Example
///
/// ## Catching panics
///
/// TODO: Implement
#[derive(Clone)]
pub struct FiberTree {
    status: Status,
    state: Option<State>, // Only `None` temporarily during state transitions.

    // Channel functionality. Fiber trees communicate with the outer world using
    // channels. Each channel is identified using an ID that is valid inside
    // this particular tree node. Channels created by the current fiber are
    // managed ("owned") by this node directly. For channels owned by the
    // outside world (such as those referenced in the environment argument),
    // this node maintains a mapping between internal and external IDs.
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
    /// This tree is currently focused on running a single fiber.
    SingleFiber(Fiber),

    /// The fiber of this tree entered a `core.parallel` scope so that it's now
    /// paused and waits for the parallel scope to end. Instead of the main
    /// former single fiber, the tree now runs the closure passed to
    /// `core.parallel` as well as any other spawned children.
    ParallelSection {
        paused_main_fiber: Fiber, // Should have Status::InParallelSection.
        nursery: ChannelId,

        /// Children and a channels where to send the result of the child. The
        /// channel's receive port is directly returned by the `core.async` function.
        /// Here, we save the ID of the channel where the result of the VM will
        /// be sent.
        children: Vec<(ChannelId, FiberTree)>,
    },
}

#[derive(Clone)]
pub enum ChannelOperation {
    Send { packet: Packet },
    Receive,
}

impl FiberTree {
    fn new_with_fiber(mut fiber: Fiber) -> Self {
        let mut tree = Self {
            status: match fiber.status {
                fiber::Status::Done => Status::Done,
                fiber::Status::Running => Status::Running,
                _ => panic!("Tried to create fiber tree with invalid fiber."),
            },
            state: None,
            internal_channels: Default::default(),
            next_internal_channel_id: 0,
            external_to_internal_channels: Default::default(),
            internal_to_external_channels: Default::default(),
            channel_operations: Default::default(),
        };
        tree.create_channel_mappings_for(&fiber.heap);
        fiber
            .heap
            .map_channel_ids(&tree.external_to_internal_channels);
        tree.state = Some(State::SingleFiber(fiber));
        tree
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
    fn is_running(&self) -> bool {
        matches!(self.status, Status::Running)
    }
    fn is_finished(&self) -> bool {
        matches!(self.status, Status::Done | Status::Panicked { .. })
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

    fn create_channel_mappings_for(&mut self, heap: &Heap) {
        for object in heap.all_objects().values() {
            if let Data::SendPort(SendPort { channel })
            | Data::ReceivePort(ReceivePort { channel }) = object.data
            {
                if !self.external_to_internal_channels.contains_key(&channel) {
                    let internal_id = self.generate_channel_id();
                    self.external_to_internal_channels
                        .insert(channel, internal_id);
                    self.internal_to_external_channels
                        .insert(internal_id, channel);
                }
            }
        }
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
                            info!("Sending packet to channel {channel}.");
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
                            info!("Sending packet to channel {channel}.");
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
                        fiber::Status::InParallelScope {
                            body,
                            return_channel,
                        } => {
                            info!("Entering parallel scope.");
                            let mut heap = Heap::default();
                            let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                            let nursery = self.generate_channel_id();
                            let nursery_send_port = heap.create_send_port(nursery);
                            let tree = FiberTree::new_for_running_closure(
                                heap,
                                use_provider,
                                body,
                                &[nursery_send_port],
                            );

                            break 'new_state State::ParallelSection {
                                paused_main_fiber: fiber,
                                nursery,
                                children: vec![(return_channel, tree)],
                            };
                        }
                        fiber::Status::Done => {
                            info!("Fiber done.");
                            self.status = Status::Done;
                        }
                        fiber::Status::Panicked { reason } => {
                            info!("Fiber panicked because of {reason}.");
                            self.status = Status::Panicked { reason };
                        }
                    }
                    State::SingleFiber(fiber)
                }
                State::ParallelSection {
                    mut paused_main_fiber,
                    nursery,
                    mut children,
                } => {
                    let (index_and_result_channel, vm) = children
                        .iter_mut()
                        .enumerate()
                        .map(|(i, (channel, vm))| (Some((i, *channel)), vm))
                        .filter(|(_, vm)| matches!(vm.status, Status::Running))
                        .choose(&mut rand::thread_rng())
                        .expect("Tried to run Vm, but no child can run.");

                    info!("Running child VM.");
                    vm.run(use_provider, num_instructions);

                    for (channel, operations) in &vm.channel_operations {
                        // TODO
                        warn!("Todo: Handle operations on channel {channel}")
                    }

                    // If this was a spawned channel and it ended execution, the result should be
                    // transmitted to the channel that's returned by the `core.async` call.
                    if let Some((index, result_channel)) = index_and_result_channel {
                        let packet = match vm.status() {
                            Status::Done => {
                                info!("Child done.");
                                let (_, vm) = children.remove(index);
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
                    if children.iter().any(|(_, vm)| vm.is_running()) {
                        self.status = Status::Running
                    }
                    if children.iter().all(|(_, vm)| vm.is_finished()) {
                        paused_main_fiber.complete_parallel_scope();
                        self.status = Status::Running;
                        break 'new_state State::SingleFiber(paused_main_fiber);
                    }

                    State::ParallelSection {
                        paused_main_fiber,
                        nursery,
                        children,
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

impl Heap {
    fn map_channel_ids(&mut self, mapping: &HashMap<ChannelId, ChannelId>) {
        for object in self.all_objects_mut().values_mut() {
            if let Data::SendPort(SendPort { channel })
            | Data::ReceivePort(ReceivePort { channel }) = &mut object.data
            {
                *channel = mapping[channel];
            }
        }
    }
}
