use super::{
    channel::{Channel, Packet},
    context::Context,
    fiber::Fiber,
    heap::{ChannelId, Data, ReceivePort, SendPort},
    tracer::Tracer,
    Closure, Heap, Pointer, TearDownResult,
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
    // status: Status,
    state: Option<State>, // Only `None` temporarily during state transitions.

    // Fiber trees communicate with the outer world using channels. Each channel
    // is identified using an ID that is valid inside this particular tree node.
    // Channels that are only used in this subtree are stored here directly.
    pub internal_channels: HashMap<ChannelId, (Channel, VecDeque<Operation>)>,
    internal_channel_id_generator: IdGenerator<ChannelId>,
    pub external_to_internal_channels: HashMap<ChannelId, ChannelId>,
    pub internal_to_external_channels: HashMap<ChannelId, ChannelId>,

    // Channel operations may be long-running and complete in any order. That's
    // why we expose operations to the parent node in this map. The parent can
    // call our `complete_` methods with an operation ID to indicate that the
    // operation is finished.
    // The parent can remove entries from this map at will. We should be able
    // to calling the `complete_*` methods. This makes it efficient to
    // propagate operations up the tree. For example, a send operation
    // containing a large value can just propagate upwards.
    // pending_operations: HashMap<OperationId, Operation>,
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
    /// 
    /// Since we only have at most one channel operation running at any given
    /// time, the only valid operation ID is 0.
    SingleFiber(Fiber),

    /// The fiber of this tree entered a `core.parallel` scope so that it's now
    /// paused and waits for the parallel scope to end. Instead of the main
    /// former single fiber, the tree now runs the closure passed to
    /// `core.parallel` as well as any other spawned children.
    ParallelSection {
        paused_main_fiber: Fiber, // Should have Status::InParallelSection.

        nursery: ChannelId,
        child_id_generator: IdGenerator<ChildId>,

        /// Children and a channels where to send the result of the child. The
        /// channel's receive port is directly returned by the `core.async` function.
        /// Here, we save the ID of the channel where the result of the VM will
        /// be sent.
        children: HashMap<ChildId, (ChannelId, FiberTree)>,

        operation_id_generator: IdGenerator<OperationId>,
        operation_id_to_child_and_its_operation_id: HashMap<OperationId, (ChildId, OperationId)>,
    },
}

type ChildId = usize;
type OperationId = usize;

#[derive(Clone)]
pub struct Operation {
    id: OperationId,
    channel: ChannelId,
    kind: OperationKind,
}
#[derive(Clone, Debug)]
pub enum OperationKind {
    Send { packet: Packet },
    Receive,
}

impl FiberTree {
    fn new_with_fiber(mut fiber: Fiber) -> Self {
        let mut tree = Self {
            // status: match fiber.status {
            //     fiber::Status::Done => Status::Done,
            //     fiber::Status::Running => Status::Running,
            //     _ => panic!("Tried to create fiber tree with invalid fiber."),
            // },
            state: None,
            internal_channels: Default::default(),
            internal_channel_id_generator: IdGenerator::new(),
            external_to_internal_channels: Default::default(),
            internal_to_external_channels: Default::default(),
            // pending_operations: Default::default(),
        };
        tree.create_channel_mappings_for(&fiber.heap);
        fiber
            .heap
            .map_channel_ids(&tree.external_to_internal_channels);
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

    pub fn status(&self) -> Status {
        self.state.as_ref().unwrap().status()
    }
    fn is_running(&self) -> bool {
        matches!(self.status(), Status::Running)
    }
    fn is_finished(&self) -> bool {
        matches!(self.status(), Status::Done | Status::Panicked { .. })
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

    fn create_channel_mappings_for(&mut self, heap: &Heap) {
        for object in heap.all_objects().values() {
            if let Data::SendPort(SendPort { channel })
            | Data::ReceivePort(ReceivePort { channel }) = object.data
            {
                if !self.external_to_internal_channels.contains_key(&channel) {
                    let internal_id = self.internal_channel_id_generator.generate();
                    self.external_to_internal_channels
                        .insert(channel, internal_id);
                    self.internal_to_external_channels
                        .insert(internal_id, channel);
                }
            }
        }
    }

    fn complete_canceling_operations(&mut self, a: Operation, b: Operation) {
        assert_eq!(a.channel, b.channel);
        match (a.kind, b.kind) {
            (OperationKind::Send { packet }, OperationKind::Receive) => {
                self.complete_send(a.id);
                self.complete_receive(b.id, packet);
            }
            (OperationKind::Receive, OperationKind::Send { packet }) => {
                self.complete_send(b.id);
                self.complete_receive(a.id, packet);
            }
            _ => panic!("operations do not cancel each other out"),
        }
    }
    fn complete_send(&mut self, id: OperationId) {
        self.state.as_mut().unwrap().complete_send(id);
    }
    fn complete_receive(&mut self, id: OperationId, packet: Packet) {
        self.state.as_mut().unwrap().complete_receive(id, packet);
    }

    pub fn run<C: Context>(&mut self, context: &mut C) -> Vec<Operation> {
        assert!(
            self.is_running(),
            "Called FiberTree::run on a tree that is not ready to run."
        );
        let mut state = mem::replace(&mut self.state, None).unwrap();
        let mut operations = vec![];
        // FIXME: Comment in before merging PR.
        // while state.is_running() && context.should_continue_running() {
            state = self.run_and_map_state(state, &mut operations, context);
        // }
        self.state = Some(state);
        debug!("Finished running tree (status = {:?}).", self.status());

        let mut external_operations = vec![];
        for operation in operations {
            let (channel, channel_operations) = match self.internal_channels.get_mut(&operation.channel) {
                Some(it) => it,
                None => {
                    if let State::ParallelSection { nursery, children , ..} = self.state.as_mut().unwrap() && operation.channel == *nursery {
                        info!("Operation is for nursery.");
                        todo!("Handle message for nursery.");
                        continue;
                    }
                    info!("Operation is for channel ch#{}, which is an external channel: {operation:?}", operation.channel);
                    todo!("Migrate channels if necessary.");
                    external_operations.push(Operation {
                            channel: self.internal_to_external_channels[&operation.channel],
                        ..operation});
                    info!("In particular, it corresponds to channel ch#{} in the outer node.", self.internal_to_external_channels[&operation.channel]);
                    continue;
                },
            };

            let was_completed = match &operation.kind {
                OperationKind::Send { packet } => {
                    if channel.send(packet.clone()) { // TODO: Don't clone
                        self.state.as_mut().unwrap().complete_send(operation.id);
                        true
                    } else {
                        false
                    }
                }
                OperationKind::Receive => {
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
                channel_operations.push_back(operation);
            }
        }

        external_operations
    }
    fn run_and_map_state<C: Context>(&mut self, state: State, operations: &mut Vec<Operation>, context: &mut C) -> State {
        match state {
            State::SingleFiber (mut fiber) => {
                debug!("Running fiber (status = {:?}).", fiber.status);

                fiber.run(context);

                match fiber.status() {
                    fiber::Status::Running => {}
                    fiber::Status::CreatingChannel { capacity } => {
                        let id = self.internal_channel_id_generator.generate();
                        self.internal_channels
                            .insert(id, (Channel::new(capacity), VecDeque::new()));
                        fiber.complete_channel_create(id);
                    }
                    fiber::Status::Sending { channel, packet } => {
                        debug!("Sending packet to channel {channel}.");
                        operations.push(Operation {
                            id: 0,
                            channel,
                            kind: OperationKind::Send { packet },
                        });
                    }
                    fiber::Status::Receiving { channel } => {
                        debug!("Receiving packet from channel {channel}.");
                        operations.push(Operation {
                            id: 0,
                            channel,
                            kind: OperationKind::Receive,
                        });
                    }
                    fiber::Status::InParallelScope {
                        body,
                        return_channel,
                    } => {
                        debug!("Entering parallel scope.");
                        let mut heap = Heap::default();
                        let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                        let nursery = self.internal_channel_id_generator.generate();
                        let nursery_send_port = heap.create_send_port(nursery);
                        let tree =
                            FiberTree::new_for_running_closure(heap, body, &[nursery_send_port]);

                        let mut fiber_id_generator = IdGenerator::start_at(1);
                        let mut children = HashMap::new();
                        children.insert(fiber_id_generator.generate(), (return_channel, tree));

                        return State::ParallelSection {
                            paused_main_fiber: fiber,
                            nursery,
                            child_id_generator: fiber_id_generator,
                            children,
                            operation_id_generator: IdGenerator::start_at(0),
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
                mut operation_id_generator,
                mut operation_id_to_child_and_its_operation_id,
                ..
            } => {
                let (child_id, result_channel, vm) = children
                    .iter_mut()
                    .map(|(id, (channel, vm))| (*id, *channel, vm))
                    .filter(|(_, _, vm)| matches!(vm.status(), Status::Running))
                    .choose(&mut rand::thread_rng())
                    .expect("Tried to run Vm, but no child can run.");

                debug!("Running child VM.");
                let new_operations = vm.run(context);

                for operation in new_operations {
                    let id = operation_id_generator.generate();
                    operation_id_to_child_and_its_operation_id.insert(id, (child_id, operation.id));
                    operations.push(Operation {
                        id,
                        ..operation
                    });
                }

                // If the child finished executing, the result should be
                // transmitted to the channel that's returned by the
                // `core.async` call.
                let packet = match vm.status() {
                    Status::Done => {
                        debug!("Child done.");
                        let (_, vm) = children.remove(&child_id).unwrap();
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
                    operations.push(Operation {
                        id: operation_id_generator.generate(),
                        channel: result_channel,
                        kind: OperationKind::Send { packet },
                    });
                }

                // Update status and state.
                if children.values().all(|(_, tree)| tree.is_finished()) {
                    paused_main_fiber.complete_parallel_scope();
                    return State::SingleFiber(paused_main_fiber);
                }

                State::ParallelSection {
                    paused_main_fiber,
                    nursery,
                    child_id_generator: fiber_id_generator,
                    children,
                    operation_id_generator,
                    operation_id_to_child_and_its_operation_id,
                }
            }
        }
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
                for (_, child) in children.values() {
                    return match child.status() {
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
                children.get_mut(&child_id).unwrap().1.complete_send(operation_id);
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
                children.get_mut(&child_id).unwrap().1.complete_receive(operation_id, packet);
            }
        }
    }
}

impl fmt::Debug for FiberTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted_channels = {
            let mut out = String::from("");
            if !self.internal_to_external_channels.is_empty() {
                out.push_str(&format!("  internal to external: {}\n", self.internal_to_external_channels.iter().map(|(internal, external)| format!("{internal}->{external}")).join(", ")));
            }
            for (id, (channel, pending_operations)) in &self.internal_channels {
                out.push_str(&format!("  ch#{id}: {channel:?}"));
                if !pending_operations.is_empty() {
                    out.push_str(&format!(" pending: "));
                    for operation in pending_operations {
                        out.push_str(&format!("{operation:?}"));
                    }
                }
                out.push('\n');
            }
            out
        };

        match self.state.as_ref().unwrap() {
            State::SingleFiber(fiber) => {
                writeln!(f, "SingleFiber {{")?;
                write!(f, "{formatted_channels}")?;
                writeln!(f, "  status: {:?}", fiber.status)?;
                write!(f, "}}")?;
            },
            State::ParallelSection { nursery, children, .. } => {
                writeln!(f, "ParallelSection {{")?;
                write!(f, "{formatted_channels}")?;
                writeln!(f, "  nursery: ch#{nursery}")?;
                for (id, (result_channel, child)) in children {
                    write!(f, "  child#{id} completing to ch#{result_channel}: ")?;
                    let child = format!("{child:?}");
                    writeln!(f, "{}\n{}", child.lines().nth(0).unwrap(), child.lines().skip(1).map(|line| format!("  {line}")).join("\n"))?;
                }
                write!(f, "}}")?;
            },
        }
        Ok(())
    }
}
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleFiber(fiber) => write!(f, "SingleFiber ({:?})", fiber.status),
            Self::ParallelSection { children, .. } => f.debug_struct("ParallelSection")
                .field("children", children).finish(),
        }
    }
}
impl fmt::Debug for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "op#{} {} ch#{}", self.id, match &self.kind {
                OperationKind::Send { packet } => format!("sending {packet:?} to"),
                OperationKind::Receive => format!("receiving from"),
        }, self.channel)
    }
}


impl Operation {
    fn cancels_out(&self, other: &Self) -> bool {
        matches!(
            (&self.kind, &other.kind),
            (OperationKind::Send { .. }, OperationKind::Receive)
                | (OperationKind::Receive, OperationKind::Send { .. })
        )
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

#[derive(Clone)]
struct IdGenerator<T: From<usize>> {
    next_id: usize,
    _data: PhantomData<T>,
}
impl<T: From<usize>> IdGenerator<T> {
    fn new() -> Self {
        Self {
            next_id: 0,
            _data: Default::default(),
        }
    }
    fn start_at(id: usize) -> Self {
        Self {
            next_id: id,
            _data: Default::default(),
        }
    }
    fn generate(&mut self) -> T {
        let id = self.next_id;
        self.next_id += 1;
        id.into()
    }
}
