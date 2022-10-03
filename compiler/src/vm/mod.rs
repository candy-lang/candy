mod builtin_functions;
mod channel;
pub mod context;
mod fiber;
mod heap;
pub mod tracer;
mod use_module;

use self::{
    channel::{ChannelBuf, Packet},
    context::{
        CombiningExecutionController, ExecutionController, RunLimitedNumberOfInstructions,
        UseProvider,
    },
    heap::{ChannelId, SendPort},
    tracer::Tracer,
};
pub use fiber::{Fiber, TearDownResult};
pub use heap::{Closure, Heap, Object, Pointer, Struct};
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    hash::Hash,
    marker::PhantomData,
};
use tracing::{info, warn};

/// A VM represents a Candy program that thinks it's currently running. Because
/// VMs are first-class Rust structs, they enable other code to store "freezed"
/// programs and to remain in control about when and for how long code runs.
#[derive(Clone)]
pub struct Vm {
    fibers: HashMap<FiberId, FiberTree>,
    root_fiber: Option<FiberId>, // only None when no fiber is created yet

    channels: HashMap<ChannelId, Channel>,
    pub external_operations: HashMap<ChannelId, Vec<Operation>>,

    fiber_id_generator: IdGenerator<FiberId>,
    channel_id_generator: IdGenerator<ChannelId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);

#[derive(Clone)]
enum FiberTree {
    /// This tree is currently focused on running a single fiber.
    Single(Single),

    /// The fiber of this tree entered a `core.parallel` scope so that it's now
    /// paused and waits for the parallel scope to end. Instead of the main
    /// former single fiber, the tree now runs the closure passed to
    /// `core.parallel` as well as any other spawned children.
    Parallel(Parallel),

    Try(Try),
}

/// Single fibers are the leafs of the fiber tree.
#[derive(Clone)]
struct Single {
    fiber: Fiber,
    parent: Option<FiberId>,
}

/// When a parallel section is entered, the fiber that started the section is
/// paused. Instead, the children of the parallel section are run. Initially,
/// there's only one child – the closure given to the parallel builtin function.
/// Using the nursery parameter (a nursery can be thought of as a pointer to a
/// parallel section), you can also spawn other fibers. In contrast to the first
/// child, those children also have an explicit send port where the closure's
/// result is sent to.
#[derive(Clone)]
struct Parallel {
    paused_fiber: Single,
    children: HashMap<FiberId, ChildKind>,
    return_value: Option<Packet>, // will later contain the body's return value
    nursery: ChannelId,
}
#[derive(Clone)]
enum ChildKind {
    InitialChild,
    SpawnedChild(ChannelId),
}

#[derive(Clone)]
struct Try {
    paused_fiber: Single,
    child: FiberId,
}

#[derive(Clone)]
enum Channel {
    Internal(InternalChannel),
    External,
    Nursery(FiberId),
}
#[derive(Clone, Debug)]
struct InternalChannel {
    buffer: ChannelBuf,
    pending_sends: VecDeque<(Option<FiberId>, Packet)>,
    pending_receives: VecDeque<Option<FiberId>>,
}

#[derive(Clone)]
pub enum Operation {
    Send {
        performing_fiber: Option<FiberId>,
        packet: Packet,
    },
    Receive {
        performing_fiber: Option<FiberId>,
    },
    Drop,
}

#[derive(Clone, Debug)]
pub enum Status {
    CanRun,
    WaitingForOperations,
    Done,
    Panicked { reason: String },
}

impl Vm {
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            fibers: HashMap::new(),
            root_fiber: None,
            external_operations: Default::default(),
            channel_id_generator: IdGenerator::start_at(0),
            fiber_id_generator: IdGenerator::start_at(0),
        }
    }

    fn set_up_with_fiber(&mut self, fiber: Fiber) {
        assert!(self.root_fiber.is_none(), "VM already set up");
        let root_fiber_id = self.fiber_id_generator.generate();
        self.fibers.insert(
            root_fiber_id,
            FiberTree::Single(Single {
                fiber,
                parent: None,
            }),
        );
        self.root_fiber = Some(root_fiber_id);
    }
    pub fn set_up_for_running_closure(
        &mut self,
        heap: Heap,
        closure: Pointer,
        arguments: &[Pointer],
    ) {
        self.set_up_with_fiber(Fiber::new_for_running_closure(heap, closure, arguments))
    }
    pub fn set_up_for_running_module_closure(&mut self, closure: Closure) {
        self.set_up_with_fiber(Fiber::new_for_running_module_closure(closure))
    }

    pub fn tear_down(mut self) -> TearDownResult {
        let tree = self.fibers.remove(&self.root_fiber.unwrap()).unwrap();
        let single = tree.into_single().unwrap();
        single.fiber.tear_down()
    }

    pub fn status(&self) -> Status {
        self.status_of(self.root_fiber.expect("VM not set up yet"))
    }
    fn status_of(&self, fiber: FiberId) -> Status {
        match &self.fibers[&fiber] {
            FiberTree::Single(Single { fiber, .. }) => match &fiber.status {
                fiber::Status::Running => Status::CanRun,
                fiber::Status::Sending { .. } | fiber::Status::Receiving { .. } => {
                    Status::WaitingForOperations
                }
                fiber::Status::CreatingChannel { .. }
                | fiber::Status::InParallelScope { .. }
                | fiber::Status::InTry { .. } => unreachable!(),
                fiber::Status::Done => Status::Done,
                fiber::Status::Panicked { reason } => Status::Panicked {
                    reason: reason.clone(),
                },
            },
            FiberTree::Parallel(Parallel { children, .. }) => {
                for child_fiber in children.keys() {
                    match self.status_of(*child_fiber) {
                        Status::CanRun => return Status::CanRun,
                        Status::WaitingForOperations => {}
                        Status::Done | Status::Panicked { .. } => unreachable!(),
                    };
                }
                // The section is still running, otherwise it would have been
                // removed. Thus, there was at least one child and all children
                // were waiting for operations.
                Status::WaitingForOperations
            }
            FiberTree::Try(Try { child, .. }) => self.status_of(*child),
        }
    }
    fn can_run(&self) -> bool {
        matches!(self.status(), Status::CanRun)
    }

    pub fn fiber(&self) -> &Fiber {
        // TODO: Remove before merging the PR
        todo!()
    }
    pub fn cloned_tracer(&self) -> Tracer {
        // TODO: Remove
        self.fiber().tracer.clone()
    }

    /// Can be called at any time from outside the VM to create a channel that
    /// can be used to communicate with the outside world.
    pub fn create_channel(&mut self) -> ChannelId {
        let id = self.channel_id_generator.generate();
        self.channels.insert(id, Channel::External);
        self.external_operations.insert(id, vec![]);
        id
    }

    pub fn complete_send(&mut self, performing_fiber: Option<FiberId>) {
        if let Some(fiber) = performing_fiber {
            let tree = self.fibers.get_mut(&fiber).unwrap();
            tree.as_single_mut().unwrap().fiber.complete_send();
        }
    }

    pub fn complete_receive(&mut self, performing_fiber: Option<FiberId>, packet: Packet) {
        if let Some(fiber) = performing_fiber {
            let tree = self.fibers.get_mut(&fiber).unwrap();
            tree.as_single_mut().unwrap().fiber.complete_receive(packet);
        }
    }

    /// May only be called if a drop operation was emitted for that channel.
    pub fn free_channel(&mut self, channel: ChannelId) {
        self.channels.remove(&channel);
        self.external_operations.remove(&channel);
    }

    pub fn run<U: UseProvider, E: ExecutionController>(
        &mut self,
        use_provider: &mut U,
        execution_controller: &mut E,
    ) {
        while self.can_run() && execution_controller.should_continue_running() {
            self.run_raw(
                use_provider,
                &mut CombiningExecutionController::new(
                    execution_controller,
                    &mut RunLimitedNumberOfInstructions::new(100),
                ),
            );
        }
    }
    fn run_raw<U: UseProvider, E: ExecutionController>(
        &mut self,
        use_provider: &mut U,
        execution_controller: &mut E,
    ) {
        assert!(
            self.can_run(),
            "Called Vm::run on a VM that is not ready to run."
        );

        // Choose a random fiber to run.
        let mut fiber_id = self.root_fiber.unwrap();
        let fiber = loop {
            match self.fibers.get_mut(&fiber_id).unwrap() {
                FiberTree::Single(Single { fiber, .. }) => break fiber,
                FiberTree::Parallel(Parallel { children, .. }) => {
                    let children_as_vec = children.iter().collect_vec();
                    let random_child = children_as_vec.choose(&mut rand::thread_rng()).unwrap();
                    fiber_id = *random_child.0
                }
                FiberTree::Try(Try { child, .. }) => fiber_id = *child,
            }
        };
        if !matches!(fiber.status(), fiber::Status::Running) {
            return;
        }

        fiber.run(use_provider, execution_controller);

        let is_finished = match fiber.status() {
            fiber::Status::Running => false,
            fiber::Status::CreatingChannel { capacity } => {
                let channel_id = self.channel_id_generator.generate();
                self.channels.insert(
                    channel_id,
                    Channel::Internal(InternalChannel {
                        buffer: ChannelBuf::new(capacity),
                        pending_sends: Default::default(),
                        pending_receives: Default::default(),
                    }),
                );
                fiber.complete_channel_create(channel_id);
                false
            }
            fiber::Status::Sending { channel, packet } => {
                self.send_to_channel(Some(fiber_id), channel, packet);
                false
            }
            fiber::Status::Receiving { channel } => {
                self.receive_from_channel(Some(fiber_id), channel);
                false
            }
            fiber::Status::InParallelScope { body } => {
                let nursery_id = self.channel_id_generator.generate();
                self.channels.insert(nursery_id, Channel::Nursery(fiber_id));

                let first_child_id = {
                    let mut heap = Heap::default();
                    let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                    let nursery_send_port = heap.create_send_port(nursery_id);
                    let id = self.fiber_id_generator.generate();
                    self.fibers.insert(
                        id,
                        FiberTree::Single(Single {
                            fiber: Fiber::new_for_running_closure(heap, body, &[nursery_send_port]),
                            parent: Some(fiber_id),
                        }),
                    );
                    id
                };

                self.fibers.replace(fiber_id, |tree| {
                    let single = tree.into_single().unwrap();
                    FiberTree::Parallel(Parallel {
                        paused_fiber: single,
                        children: [(first_child_id, ChildKind::InitialChild)]
                            .into_iter()
                            .collect(),
                        return_value: None,
                        nursery: nursery_id,
                    })
                });

                false
            }
            fiber::Status::InTry { body } => {
                let child_id = {
                    let mut heap = Heap::default();
                    let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                    let id = self.fiber_id_generator.generate();
                    self.fibers.insert(
                        id,
                        FiberTree::Single(Single {
                            fiber: Fiber::new_for_running_closure(heap, body, &[]),
                            parent: Some(fiber_id),
                        }),
                    );
                    id
                };

                self.fibers.replace(fiber_id, |tree| {
                    let single = tree.into_single().unwrap();
                    FiberTree::Try(Try {
                        paused_fiber: single,
                        child: child_id,
                    })
                });

                false
            }
            fiber::Status::Done => {
                info!("A fiber is done.");
                true
            }
            fiber::Status::Panicked { reason } => {
                warn!("A fiber panicked because {reason}.");
                true
            }
        };

        if is_finished && fiber_id != self.root_fiber.unwrap() {
            let single = self
                .fibers
                .remove(&fiber_id)
                .unwrap()
                .into_single()
                .unwrap();
            let TearDownResult { heap, result, .. } = single.fiber.tear_down();
            let parent = single
                .parent
                .expect("we already checked we're not the root fiber");

            match self.fibers.get_mut(&parent).unwrap() {
                FiberTree::Single(_) => unreachable!("single fibers can't have children"),
                FiberTree::Parallel(parallel) => {
                    let child = parallel.children.remove(&fiber_id).unwrap();

                    match result {
                        Ok(return_value) => {
                            let is_finished = parallel.children.is_empty();
                            let packet = Packet {
                                heap,
                                value: return_value,
                            };
                            match child {
                                ChildKind::InitialChild => parallel.return_value = Some(packet),
                                ChildKind::SpawnedChild(return_channel) => {
                                    self.send_to_channel(None, return_channel, packet)
                                }
                            }

                            if is_finished {
                                self.finish_parallel(parent, Ok(()))
                            }
                        }
                        Err(panic_reason) => self.finish_parallel(parent, Err(panic_reason)),
                    }
                }
                FiberTree::Try(Try { .. }) => {
                    self.fibers.replace(parent, |tree| {
                        let mut paused_fiber = tree.into_try().unwrap().paused_fiber;
                        paused_fiber
                            .fiber
                            .complete_try(result.map(|value| Packet { heap, value }));
                        FiberTree::Single(paused_fiber)
                    });
                }
            }
        }

        let all_channels = self.channels.keys().copied().collect::<HashSet<_>>();
        let mut known_channels = HashSet::new();
        for fiber in self.fibers.values() {
            if let Some(single) = fiber.as_single() {
                known_channels.extend(single.fiber.heap.known_channels().into_iter());
            }
        }
        let forgotten_channels = all_channels.difference(&known_channels);
        for channel in forgotten_channels {
            match self.channels.get(channel).unwrap() {
                // If an internal channel is not referenced by any fiber, no
                // reference to it can be obtained in the future. Thus, it's
                // safe to remove such channels.
                Channel::Internal(_) => {
                    self.channels.remove(channel);
                }
                // External channels may be re-sent into the VM from the outside
                // even after no fibers remember them. Rather than removing them
                // directly, we communicate to the outside that no fiber
                // references them anymore. The outside can then call
                // `free_channel` when it doesn't intend to re-use the channel.
                Channel::External => {
                    self.push_external_operation(*channel, Operation::Drop);
                }
                // Nurseries are automatically removed when they are exited.
                Channel::Nursery(_) => {}
            }
        }
    }
    fn finish_parallel(&mut self, parallel_id: FiberId, result: Result<(), String>) {
        let parallel = self
            .fibers
            .get_mut(&parallel_id)
            .unwrap()
            .as_parallel_mut()
            .unwrap();

        for child_id in parallel.children.clone().into_keys() {
            self.cancel(child_id);
        }

        self.fibers.replace(parallel_id, |tree| {
            let Parallel {
                mut paused_fiber,
                nursery,
                ..
            } = tree.into_parallel().unwrap();
            self.channels.remove(&nursery).unwrap();
            paused_fiber.fiber.complete_parallel_scope(result);
            FiberTree::Single(paused_fiber)
        });
    }
    fn cancel(&mut self, fiber: FiberId) {
        match self.fibers.remove(&fiber).unwrap() {
            FiberTree::Single(_) => {}
            FiberTree::Parallel(Parallel {
                children, nursery, ..
            }) => {
                self.channels
                    .remove(&nursery)
                    .unwrap()
                    .to_nursery()
                    .unwrap();
                for child_fiber in children.keys() {
                    self.cancel(*child_fiber);
                }
            }
            FiberTree::Try(Try { child, .. }) => self.cancel(child),
        }
    }

    fn send_to_channel(
        &mut self,
        performing_fiber: Option<FiberId>,
        channel_id: ChannelId,
        packet: Packet,
    ) {
        let channel = match self.channels.get_mut(&channel_id) {
            Some(channel) => channel,
            None => {
                // The channel was a nursery that died.
                if let Some(fiber) = performing_fiber {
                    let tree = self.fibers.get_mut(&fiber).unwrap();
                    tree.as_single_mut().unwrap().fiber.panic(
                        "the nursery is already dead because the parallel section ended"
                            .to_string(),
                    );
                }
                return;
            }
        };
        match channel {
            Channel::Internal(channel) => {
                channel.send(&mut self.fibers, performing_fiber, packet);
            }
            Channel::External => self.push_external_operation(
                channel_id,
                Operation::Send {
                    performing_fiber,
                    packet,
                },
            ),
            Channel::Nursery(parent_id) => {
                info!("Nursery received packet {:?}", packet);
                let parent_id = *parent_id;

                match Self::parse_spawn_packet(packet) {
                    Some((heap, closure_to_spawn, return_channel)) => {
                        let child_id = self.fiber_id_generator.generate();
                        self.fibers.insert(
                            child_id,
                            FiberTree::Single(Single {
                                fiber: Fiber::new_for_running_closure(heap, closure_to_spawn, &[]),
                                parent: Some(parent_id),
                            }),
                        );

                        self.fibers
                            .get_mut(&parent_id)
                            .unwrap()
                            .as_parallel_mut()
                            .unwrap()
                            .children
                            .insert(child_id, ChildKind::SpawnedChild(return_channel));
                    }
                    None => self.finish_parallel(
                        parent_id,
                        Err("a nursery received an invalid message".to_string()),
                    ),
                }

                InternalChannel::complete_send(&mut self.fibers, performing_fiber);
            }
        }
    }
    fn parse_spawn_packet(packet: Packet) -> Option<(Heap, Pointer, ChannelId)> {
        let Packet { mut heap, value } = packet;
        let arguments: Struct = heap.get(value).data.clone().try_into().ok()?;

        let closure_symbol = heap.create_symbol("Closure".to_string());
        let closure_address = arguments.get(&heap, closure_symbol)?;
        let closure: Closure = heap.get(closure_address).data.clone().try_into().ok()?;
        if closure.num_args > 0 {
            return None;
        }

        let return_channel_symbol = heap.create_symbol("ReturnChannel".to_string());
        let return_channel_address = arguments.get(&heap, return_channel_symbol)?;
        let return_channel: SendPort = heap
            .get(return_channel_address)
            .data
            .clone()
            .try_into()
            .ok()?;

        Some((heap, closure_address, return_channel.channel))
    }

    fn receive_from_channel(&mut self, performing_fiber: Option<FiberId>, channel: ChannelId) {
        match self.channels.get_mut(&channel).unwrap() {
            Channel::Internal(channel) => {
                channel.receive(&mut self.fibers, performing_fiber);
            }
            Channel::External => {
                self.push_external_operation(channel, Operation::Receive { performing_fiber });
            }
            Channel::Nursery { .. } => unreachable!("nurseries are only sent stuff"),
        }
    }

    fn push_external_operation(&mut self, channel: ChannelId, operation: Operation) {
        self.external_operations
            .entry(channel)
            .or_default()
            .push(operation);
    }
}

impl InternalChannel {
    fn send(
        &mut self,
        fibers: &mut HashMap<FiberId, FiberTree>,
        performing_fiber: Option<FiberId>,
        packet: Packet,
    ) {
        self.pending_sends.push_back((performing_fiber, packet));
        self.work_on_pending_operations(fibers);
    }

    fn receive(
        &mut self,
        fibers: &mut HashMap<FiberId, FiberTree>,
        performing_fiber: Option<FiberId>,
    ) {
        self.pending_receives.push_back(performing_fiber);
        self.work_on_pending_operations(fibers);
    }

    fn work_on_pending_operations(&mut self, fibers: &mut HashMap<FiberId, FiberTree>) {
        if self.buffer.capacity == 0 {
            while !self.pending_sends.is_empty() && !self.pending_receives.is_empty() {
                let (send_id, packet) = self.pending_sends.pop_front().unwrap();
                let receive_id = self.pending_receives.pop_front().unwrap();
                Self::complete_send(fibers, send_id);
                Self::complete_receive(fibers, receive_id, packet);
            }
        } else {
            loop {
                let mut did_perform_operation = false;

                if !self.buffer.is_full() && let Some((fiber, packet)) = self.pending_sends.pop_front() {
                    self.buffer.send(packet);
                    Self::complete_send(fibers, fiber);
                    did_perform_operation = true;
                }

                if !self.buffer.is_empty() && let Some(fiber) = self.pending_receives.pop_front() {
                    let packet = self.buffer.receive();
                    Self::complete_receive(fibers, fiber, packet);
                    did_perform_operation = true;
                }

                if !did_perform_operation {
                    break;
                }
            }
        }
    }

    fn complete_send(fibers: &mut HashMap<FiberId, FiberTree>, fiber: Option<FiberId>) {
        if let Some(fiber) = fiber {
            let fiber = fibers.get_mut(&fiber).unwrap().as_single_mut().unwrap();
            fiber.fiber.complete_send();
        }
    }
    fn complete_receive(
        fibers: &mut HashMap<FiberId, FiberTree>,
        fiber: Option<FiberId>,
        packet: Packet,
    ) {
        if let Some(fiber) = fiber {
            let fiber = fibers.get_mut(&fiber).unwrap().as_single_mut().unwrap();
            fiber.fiber.complete_receive(packet);
        }
    }
}

impl Channel {
    fn to_nursery(&self) -> Option<FiberId> {
        match self {
            Channel::Nursery(fiber) => Some(*fiber),
            _ => None,
        }
    }
}
impl FiberTree {
    fn into_single(self) -> Option<Single> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }
    fn as_single(&self) -> Option<&Single> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }
    fn as_single_mut(&mut self) -> Option<&mut Single> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }

    fn into_parallel(self) -> Option<Parallel> {
        match self {
            FiberTree::Parallel(parallel) => Some(parallel),
            _ => None,
        }
    }
    fn as_parallel_mut(&mut self) -> Option<&mut Parallel> {
        match self {
            FiberTree::Parallel(parallel) => Some(parallel),
            _ => None,
        }
    }

    fn into_try(self) -> Option<Try> {
        match self {
            FiberTree::Try(try_) => Some(try_),
            _ => None,
        }
    }
}

impl fmt::Debug for Vm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vm")
            .field("fibers", &self.fibers)
            .field("channels", &self.channels)
            .field("external_operations", &self.external_operations)
            .finish()
    }
}
impl fmt::Debug for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fiber_{:x}", self.0)
    }
}
impl fmt::Debug for FiberTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(Single { fiber, parent }) => f
                .debug_struct("SingleFiber")
                .field("status", &fiber.status())
                .field("parent", parent)
                .finish(),
            Self::Parallel(Parallel {
                children, nursery, ..
            }) => f
                .debug_struct("ParallelSection")
                .field("children", children)
                .field("nursery", nursery)
                .finish(),
            Self::Try(Try { child, .. }) => f.debug_struct("Try").field("child", child).finish(),
        }
    }
}
impl fmt::Debug for ChildKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChildKind::InitialChild => write!(f, "is initial child"),
            ChildKind::SpawnedChild(return_channel) => write!(f, "returns to {:?}", return_channel),
        }
    }
}
impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(InternalChannel {
                buffer,
                pending_sends,
                pending_receives,
            }) => f
                .debug_struct("InternalChannel")
                .field("buffer", buffer)
                .field(
                    "operations",
                    &pending_sends
                        .iter()
                        .map(|(fiber, packet)| Operation::Send {
                            performing_fiber: *fiber,
                            packet: packet.clone(),
                        })
                        .chain(pending_receives.iter().map(|fiber| Operation::Receive {
                            performing_fiber: *fiber,
                        }))
                        .collect_vec(),
                )
                .finish(),
            Self::External => f.debug_tuple("External").finish(),
            Self::Nursery(fiber) => f.debug_tuple("Nursery").field(fiber).finish(),
        }
    }
}
impl fmt::Debug for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Operation::Send {
                performing_fiber,
                packet,
            } => {
                if let Some(fiber) = performing_fiber {
                    write!(f, "{:?} ", fiber)?;
                }
                write!(f, "sending {:?}", packet)
            }
            Operation::Receive { performing_fiber } => {
                if let Some(fiber) = performing_fiber {
                    write!(f, "{:?} ", fiber)?;
                }
                write!(f, "receiving")
            }
            Operation::Drop => write!(f, "dropping"),
        }
    }
}

impl From<usize> for FiberId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

#[derive(Clone)]
struct IdGenerator<T: From<usize>> {
    next_id: usize,
    _data: PhantomData<T>,
}
impl<T: From<usize>> IdGenerator<T> {
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

trait ReplaceHashMapValue<K, V> {
    fn replace<F: FnOnce(V) -> V>(&mut self, key: K, replacer: F);
}
impl<K: Eq + Hash, V> ReplaceHashMapValue<K, V> for HashMap<K, V> {
    fn replace<F: FnOnce(V) -> V>(&mut self, key: K, replacer: F) {
        let value = self.remove(&key).unwrap();
        let value = replacer(value);
        self.insert(key, value);
    }
}
