use super::{
    channel::{ChannelBuf, Packet},
    context::Context,
    fiber::Fiber,
    heap::{ChannelId, Data, ReceivePort, SendPort},
    tracer::Tracer,
    Closure, Heap, Pointer, TearDownResult, utils::IdGenerator,
};
use crate::vm::fiber;
use itertools::Itertools;
use rand::seq::IteratorRandom;
use core::fmt;
use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    mem,
};
use tracing::{debug, info, warn};

/// A fiber tree a Candy program that thinks it's currently running. Everything
/// from a single fiber to a whole program spanning multiple nested parallel
/// scopes is represented as a fiber tree. Because fiber trees are first-class
/// Rust structs, they enable other code to store "freezed" programs and to
/// remain in control about when and for how long code runs.
///
/// While fibers are "pure" virtual machines that manage a heap and stack, fiber
/// _trees_ encapsulate fibers and manage channels that are used by them.
///
/// ## A Tree of Fibers
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
    state: Option<State>, // Only `None` temporarily during state transitions.
    internal_channels: HashMap<ChannelId, InternalChannel>,
    operation_id_generator: IdGenerator<OperationId>,
    ongoing_migrations: HashMap<OperationId, ChannelId>,
    operations_dependent_on_migration: HashMap<ChannelId, Vec<Operation>>,
}

#[derive(Clone)]
struct InternalChannel {
    buffer: ChannelBuf,
    pending_operations: VecDeque<Operation>,
}

#[derive(Clone)]
enum State {
    /// This tree is currently focused on running a single fiber.
    /// 
    /// Since we only have at most one channel operation running at any given
    /// time, the only valid operation ID is 0.
    SingleFiber(Fiber),

    /// The original single fiber of this tree entered a `core.parallel` scope
    /// so that it's now paused and waits for the parallel scope to end. Instead
    /// of the original fiber, the tree now runs the closure passed to
    /// `core.parallel` as well as any other spawned children.
    ParallelSection {
        paused_main_fiber: Fiber, // Should have `Status::InParallelSection`.

        nursery: ChannelId,
        child_id_generator: IdGenerator<ChildId>,
        children: HashMap<ChildId, Child>,

        // We forward operations that our children want to perform and we can't
        // handle ourselves to our parent. This map allows us to dispatch
        // completions of those operations to the correct child.
        operation_id_to_child_and_its_operation_id: HashMap<OperationId, (ChildId, OperationId)>,
    },
}

#[derive(Clone)]
struct Child {
    tree: FiberTree,

    /// When the `tree` finishes running without panicking, its return value
    /// will be sent to this channel. The channel's send port is not exposed in
    /// Candy code, but its receive port is returned by the `core.async`
    /// function.
    channel_for_completion: ChannelId,
}

pub type ChildId = usize;
type OperationId = usize;

#[derive(Clone, Debug)]
pub struct Operation {
    id: OperationId,
    kind: OperationKind,
}
#[derive(Clone, Debug)]
pub enum OperationKind {
    Send { channel: ChannelId, packet: Packet },
    Receive { channel: ChannelId },
    MigrateOut,
    // Drop { channel: Channel },
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    WaitingForOperations,
    Done,
    Panicked { reason: String },
}

impl FiberTree {
    fn new_with_fiber(mut fiber: Fiber) -> Self {
        let mut tree = Self {
            state: None,
            internal_channels: Default::default(),
            operation_id_generator: IdGenerator::start_at(0),
            ongoing_migrations: Default::default(),
            operations_dependent_on_migration: Default::default(),
        };
        tree.state = Some(State::SingleFiber(fiber));
        tree
    }
    pub fn new_for_running_closure(heap: Heap, closure: Pointer, arguments: &[Pointer]) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_closure(heap, closure, arguments))
    }
    pub fn new_for_running_module_closure(closure: Closure) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_module_closure(closure))
    }
    pub fn tear_down(self) -> TearDownResult {
        debug!("FiberTree::tear_down called (our status = {:?}).", self.status());
        match self.into_state() {
            State::SingleFiber(fiber) => fiber.tear_down(),
            State::ParallelSection { .. } => {
                panic!("Called `Vm::tear_down` while in parallel scope")
            }
        }
    }

    pub fn fiber(&self) -> &Fiber { // TODO: Remove before merging the PR
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

    fn into_state(self) -> State {
        self.state
            .expect("Tried to get tree state during state transition")
    }
    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("Tried to get tree state during state transition")
    }
    fn state_mut(&mut self) -> &mut State {
        self.state
            .as_mut()
            .expect("Tried to get tree state during state transition")
    }

    fn is_internal(&self, channel: ChannelId) -> bool {
        if self.internal_channels.contains_key(&channel) {
            true
        } else if let State::ParallelSection { nursery, .. } = self.state() {
            *nursery == channel
        } else {
            false
        }
    }

    pub fn status(&self) -> Status {
        self.state().status()
    }
    fn is_running(&self) -> bool {
        self.state().is_running()
    }
    fn is_finished(&self) -> bool {
        self.state().is_finished()
    }
}
impl State {
    pub fn status(&self) -> Status {
        match self {
            State::SingleFiber(fiber) => match &fiber.status {
                fiber::Status::Running => Status::Running,
                fiber::Status::Sending { .. } |
                fiber::Status::Receiving { .. } => Status::WaitingForOperations,
                fiber::Status::CreatingChannel { .. } |
                fiber::Status::InParallelScope { .. } => unreachable!(),
                fiber::Status::Done => Status::Done,
                fiber::Status::Panicked { reason } => Status::Panicked { reason: reason.clone() },
            },
            State::ParallelSection { children, .. } => {
                for child in children.values() {
                    return match child.tree.status() {
                        Status::Running => Status::Running,
                        Status::WaitingForOperations => Status::WaitingForOperations,
                        Status::Done => continue,
                        Status::Panicked { reason } => Status::Panicked { reason },
                    };
                }
                unreachable!("We should have exited the parallel section")
            },
        }
    }
    fn is_running(&self) -> bool {
        matches!(self.status(), Status::Running)
    }
    fn is_finished(&self) -> bool {
        matches!(self.status(), Status::Done | Status::Panicked { .. })
    }
}

impl FiberTree {
    pub fn run<C: Context>(&mut self, context: &mut C, channel_id_generator: &mut IdGenerator<ChannelId>) -> Vec<Operation> {
        assert!(
            self.is_running(),
            "Called FiberTree::run on a tree that is not ready to run."
        );
        let mut state = mem::replace(&mut self.state, None).unwrap();
        let mut operations = vec![];
        // FIXME: Comment in before merging PR.
        // while state.is_running() && context.should_continue_running() {
            state = self.run_and_map_state(state, &mut operations, context, channel_id_generator);
        // }
        self.state = Some(state);
        debug!("Finished running tree (status = {:?}).", self.status());

        let mut external_operations = vec![];

        fn push_external_operation(this: &mut FiberTree, channel_id_generator: &mut IdGenerator<ChannelId>, external_operations: &mut Vec<Operation>, operation: Operation) {
            let used_channels = match &operation.kind {
                OperationKind::Send { channel, packet } => {
                    let mut out = vec![*channel];
                    packet.collect_channel_ids(&mut out);
                    out
                },
                OperationKind::Receive { channel } => vec![*channel],
                OperationKind::MigrateOut => vec![],
            };
            let internal_channels = used_channels.into_iter().filter(|channel| this.is_internal(*channel)).collect_vec();
            for channel in &internal_channels {
                let operation_id = channel_id_generator.generate();
                this.ongoing_migrations.insert(operation_id, *channel);
                external_operations.push(Operation {
                    id: operation_id,
                    kind: OperationKind::MigrateOut,
                });
            }
            if !internal_channels.is_empty() {
                this.operations_dependent_on_migration.entry(internal_channels[0]).or_default().push(operation);
            }
        }

        for operation in operations {
            let channel = match operation.kind {
                OperationKind::MigrateOut => {
                    let id = channel_id_generator.generate();
                    match self.state_mut() {
                        State::SingleFiber(_) => unreachable!("Single fibers should never have to migrate out channels."),
                        State::ParallelSection { children, operation_id_to_child_and_its_operation_id, .. } => {
                            let (child_id, operation_id) = operation_id_to_child_and_its_operation_id[&id];
                            let channel = children.get_mut(&child_id).unwrap().tree.migrate_out(operation_id, id);
                            self.internal_channels.insert(id, InternalChannel { buffer: channel, pending_operations: Default::default() });
                        },
                    }
                    continue;
                }
                OperationKind::Send { channel, .. } => channel,
                OperationKind::Receive { channel } => channel,
            };

            let (channel, pending_operations) = match self.internal_channels.get_mut(&channel) {
                Some(InternalChannel { buffer, pending_operations }) => (buffer, pending_operations),
                None => {
                    if let State::ParallelSection { nursery, children , ..} = self.state.as_mut().unwrap() && channel == *nursery {
                        info!("Operation is for nursery.");
                        todo!("Handle message for nursery.");
                        continue;
                    }
                    info!("Operation is for channel ch#{}, which is an external channel: {operation:?}", channel);
                    push_external_operation(self, channel_id_generator, &mut external_operations, operation);
                    continue;
                },
            };

            let was_completed = match &operation.kind {
                OperationKind::MigrateOut => unreachable!("handled above"),
                OperationKind::Send { packet, .. } => {
                    if channel.send(packet.clone()) { // TODO: Don't clone
                        self.state.as_mut().unwrap().complete_send(operation.id);
                        true
                    } else {
                        false
                    }
                }
                OperationKind::Receive {..}=> {
                    if let Some(packet) = channel.receive() {
                        self.state.as_mut().unwrap().complete_receive(operation.id, packet);
                        true
                    } else {
                        false
                    }
                }
            };

            // TODO: Try canceling out with first operation.
            // if let Some(operation) = operations.front() && new_operation.cancels_out(operation) {
            //     let operation = operations.pop_front().unwrap();
            //     self.complete_canceling_operations(operation, new_operation);
            //     return;
            // }

            if was_completed {
                // TODO: Try completing more operations if that succeeded.
            } else {
                pending_operations.push_back(operation);
            }
        }

        external_operations
    }
    fn run_and_map_state<C: Context>(&mut self, state: State, operations: &mut Vec<Operation>, context: &mut C, channel_id_generator: &mut IdGenerator<ChannelId>) -> State {
        match state {
            State::SingleFiber (mut fiber) => {
                debug!("Running fiber (status = {:?}).", fiber.status);

                fiber.run(context);

                match fiber.status() {
                    fiber::Status::Running => {}
                    fiber::Status::CreatingChannel { capacity } => {
                        let id = channel_id_generator.generate();
                        self.internal_channels
                            .insert(id, InternalChannel {
                                buffer: ChannelBuf::new(capacity),
                                pending_operations: VecDeque::new(),
                            });
                        fiber.complete_channel_create(id);
                    }
                    fiber::Status::Sending { channel, packet } => {
                        debug!("Sending packet to channel {channel}.");
                        operations.push(Operation {
                            id: 0,
                            kind: OperationKind::Send { channel, packet },
                        });
                    }
                    fiber::Status::Receiving { channel } => {
                        debug!("Receiving packet from channel {channel}.");
                        operations.push(Operation {
                            id: 0,
                            kind: OperationKind::Receive { channel },
                        });
                    }
                    fiber::Status::InParallelScope {
                        body,
                        return_channel,
                    } => {
                        debug!("Entering parallel scope.");
                        let mut heap = Heap::default();
                        let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                        let nursery = channel_id_generator.generate();
                        let nursery_send_port = heap.create_send_port(nursery);
                        let tree =
                            FiberTree::new_for_running_closure(heap, body, &[nursery_send_port]);

                        let mut fiber_id_generator = IdGenerator::start_at(1);
                        let mut children = HashMap::new();
                        children.insert(fiber_id_generator.generate(),  Child {
                            tree,channel_for_completion: return_channel, });

                        return State::ParallelSection {
                            paused_main_fiber: fiber,
                            nursery,
                            child_id_generator: fiber_id_generator,
                            children,
                            operation_id_to_child_and_its_operation_id: Default::default(),
                        };
                    }
                    fiber::Status::Done => info!("Fiber done."),
                    fiber::Status::Panicked { reason } => {
                        debug!("Fiber panicked because of {reason}.")
                    }
                }
                State::SingleFiber(fiber)
            }
            State::ParallelSection {
                mut paused_main_fiber,
                nursery,
                child_id_generator: fiber_id_generator,
                mut children,
                mut operation_id_to_child_and_its_operation_id,
                ..
            } => {
                
                let (child_id, child) = children
                    .iter_mut()
                    .map(|(id, child)| (*id, child))
                    .filter(|(_, child)| matches!(child.tree.status(), Status::Running))
                    .choose(&mut rand::thread_rng())
                    .expect("Tried to run Vm, but no child can run.");
                let channel_for_completion = child.channel_for_completion;

                debug!("Running child VM.");
                let new_operations = child.tree.run(context, channel_id_generator);

                for operation in new_operations {
                    let id = self.operation_id_generator.generate();
                    operation_id_to_child_and_its_operation_id.insert(id, (child_id, operation.id));
                    operations.push(Operation {
                        id,
                        ..operation
                    });
                }

                // If the child finished executing, the result should be
                // transmitted to the channel that's returned by the
                // `core.async` call.
                let packet = match child.tree.status() {
                    Status::Done => {
                        debug!("Child done.");
                        let child = children.remove(&child_id).unwrap();
                        let TearDownResult {
                            heap: vm_heap,
                            result,
                            ..
                        } = child.tree.tear_down();
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
                    operations.push(Operation {
                        id: self.operation_id_generator.generate(),
                        kind: OperationKind::Send { channel: channel_for_completion, packet },
                    });
                }

                if children.values().all(|child| child.tree.is_finished()) {
                    paused_main_fiber.complete_parallel_scope();
                    return State::SingleFiber(paused_main_fiber);
                }

                State::ParallelSection {
                    paused_main_fiber,
                    nursery,
                    child_id_generator: fiber_id_generator,
                    children,
                    operation_id_to_child_and_its_operation_id,
                }
            }
        }
    }

    fn migrate_out(&mut self, id: OperationId, external_id: ChannelId) -> ChannelBuf {
        todo!()
    }
    fn complete_send(&mut self, id: OperationId) {
        self.state_mut().complete_send(id);
    }
    fn complete_receive(&mut self, id: OperationId, packet: Packet) {
        self.state_mut().complete_receive(id, packet);
    }
}

impl State {
    fn complete_send(&mut self, id: OperationId) {
        debug!("Completing send {id}.");
        match self {
            State::SingleFiber(fiber) => {
                assert_eq!(id, 0);
                fiber.complete_send();
            }
            State::ParallelSection {
                children,
                operation_id_to_child_and_its_operation_id,
                ..
            } => {
                let (child_id, operation_id) = operation_id_to_child_and_its_operation_id[&id];
                children.get_mut(&child_id).unwrap().tree.complete_send(operation_id);
            }
        }
    }
    fn complete_receive(&mut self, id: OperationId, packet: Packet) {
        debug!("Completing receive {id}.");
        match self {
            State::SingleFiber (fiber)  => {
                assert_eq!(id, 0);
                fiber.complete_receive(packet);
            }
            State::ParallelSection {
                children,
                operation_id_to_child_and_its_operation_id,
                ..
            } => {
                let (child_id, operation_id) = operation_id_to_child_and_its_operation_id[&id];
                children.get_mut(&child_id).unwrap().tree.complete_receive(operation_id, packet);
            }
        }
    }
}

impl Packet {
    fn collect_channel_ids(&self, out: &mut Vec<ChannelId>) {
        self.value.collect_channel_ids(&self.heap, out)
    }
}
impl Pointer {
    fn collect_channel_ids(&self, heap: &Heap, out: &mut Vec<ChannelId>) {
        let object = heap.get(*self);
        if let Data::SendPort(SendPort {channel }) | Data::ReceivePort(ReceivePort { channel }) = object.data {
            out.push(channel);
        }
        for address in object.children() {
            address.collect_channel_ids(heap, out);
        }
    }
}
