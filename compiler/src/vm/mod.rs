mod builtin_functions;
mod channel;
pub mod context;
mod fiber;
mod heap;
pub mod tracer;
mod use_module;

use self::{
    channel::{ChannelBuf, Packet},
    context::Context,
    heap::{ChannelId, SendPort},
    tracer::Tracer,
};
pub use fiber::{Fiber, TearDownResult};
pub use heap::{Closure, Heap, Object, Pointer, Struct};
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::{
    collections::{HashMap, VecDeque},
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
    root_fiber: FiberId,

    // TODO: Drop channels
    channels: HashMap<ChannelId, Channel>,
    pub external_operations: HashMap<ChannelId, Vec<Operation>>,

    fiber_id_generator: IdGenerator<FiberId>,
    channel_id_generator: IdGenerator<ChannelId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FiberId(usize);

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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct OperationId(usize);

#[derive(Clone)]
pub struct Operation {
    performing_fiber: Option<FiberId>,
    kind: OperationKind,
}
#[derive(Clone, Debug)]
pub enum OperationKind {
    Send { packet: Packet },
    Receive,
}

#[derive(Clone, Debug)]
pub enum Status {
    CanRun,
    WaitingForOperations,
    Done,
    Panicked { reason: String },
}

impl Vm {
    fn new_with_fiber(fiber: Fiber) -> Self {
        let fiber = FiberTree::Single(Single {
            fiber,
            parent: None,
        });
        let mut fiber_id_generator = IdGenerator::start_at(0);
        let root_fiber_id = fiber_id_generator.generate();
        Self {
            channels: Default::default(),
            fibers: [(root_fiber_id, fiber)].into_iter().collect(),
            root_fiber: root_fiber_id,
            external_operations: Default::default(),
            channel_id_generator: IdGenerator::start_at(0),
            fiber_id_generator,
        }
    }
    pub fn new_for_running_closure(heap: Heap, closure: Pointer, arguments: &[Pointer]) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_closure(heap, closure, arguments))
    }
    pub fn new_for_running_module_closure(closure: Closure) -> Self {
        Self::new_with_fiber(Fiber::new_for_running_module_closure(closure))
    }
    pub fn tear_down(mut self) -> TearDownResult {
        let tree = self.fibers.remove(&self.root_fiber).unwrap();
        let single = tree.into_single().unwrap();
        single.fiber.tear_down()
    }

    pub fn status(&self) -> Status {
        self.status_of(self.root_fiber)
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
    fn is_running(&self) -> bool {
        matches!(self.status(), Status::CanRun)
    }
    fn is_finished(&self) -> bool {
        matches!(self.status(), Status::Done | Status::Panicked { .. })
    }

    pub fn fiber(&self) -> &Fiber {
        // TODO: Remove before merging the PR
        todo!()
    }
    pub fn cloned_tracer(&self) -> Tracer {
        self.fiber().tracer.clone()
    }

    pub fn create_channel(&mut self) -> ChannelId {
        let id = self.channel_id_generator.generate();
        self.channels.insert(id, Channel::External);
        id
    }

    pub fn run<C: Context>(&mut self, context: &mut C) {
        assert!(
            self.is_running(),
            "Called Vm::run on a VM that is not ready to run."
        );

        let mut fiber_id = self.root_fiber;
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

        // TODO: Limit context.
        fiber.run(context);

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

        if is_finished && fiber_id != self.root_fiber {
            let tree = self.fibers.remove(&fiber_id).unwrap();
            let single = tree.into_single().unwrap();
            let TearDownResult { heap, result, .. } = single.fiber.tear_down();

            let parent_id = single
                .parent
                .expect("we already checked we're not the root fiber");
            match self.fibers.get_mut(&parent_id).unwrap() {
                FiberTree::Single(_) => unreachable!(),
                FiberTree::Parallel(parallel) => {
                    let child = parallel.children.remove(&fiber_id).unwrap();
                    let nursery = parallel.nursery;

                    let result_of_parallel = match result {
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
                                Some(Ok(()))
                            } else {
                                None
                            }
                        }
                        Err(panic_reason) => {
                            for fiber_id in parallel.children.clone().into_keys() {
                                self.cancel(fiber_id);
                            }
                            Some(Err(panic_reason))
                        }
                    };

                    if let Some(result) = result_of_parallel {
                        self.channels.remove(&nursery).unwrap();
                        self.fibers.replace(parent_id, |tree| {
                            let mut paused_fiber = tree.into_parallel().unwrap().paused_fiber;
                            paused_fiber.fiber.complete_parallel_scope(result);
                            FiberTree::Single(paused_fiber)
                        });
                    }
                }
                FiberTree::Try(Try { .. }) => {
                    self.fibers.replace(parent_id, |tree| {
                        let mut paused_fiber = tree.into_try().unwrap().paused_fiber;
                        paused_fiber
                            .fiber
                            .complete_try(result.map(|value| Packet { heap, value }));
                        FiberTree::Single(paused_fiber)
                    });
                }
            }
        }
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
        channel: ChannelId,
        packet: Packet,
    ) {
        match self.channels.get_mut(&channel).unwrap() {
            Channel::Internal(channel) => {
                channel.send(&mut self.fibers, performing_fiber, packet);
            }
            Channel::External => self.push_external_operation(
                channel,
                Operation {
                    performing_fiber,
                    kind: OperationKind::Send { packet },
                },
            ),
            Channel::Nursery(parent_id) => {
                info!("Nursery received packet {:?}", packet);
                let (heap, closure_to_spawn, return_channel) =
                    match Self::parse_spawn_packet(packet) {
                        Some(it) => it,
                        None => {
                            // The nursery received an invalid message. TODO: Handle this.
                            panic!("A nursery received an invalid message.");
                        }
                    };
                let child_id = self.fiber_id_generator.generate();
                self.fibers.insert(
                    child_id,
                    FiberTree::Single(Single {
                        fiber: Fiber::new_for_running_closure(heap, closure_to_spawn, &[]),
                        parent: Some(*parent_id),
                    }),
                );

                let parent = self
                    .fibers
                    .get_mut(parent_id)
                    .unwrap()
                    .as_parallel_mut()
                    .unwrap();
                parent
                    .children
                    .insert(child_id, ChildKind::SpawnedChild(return_channel));

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
                self.push_external_operation(
                    channel,
                    Operation {
                        performing_fiber,
                        kind: OperationKind::Receive,
                    },
                );
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
                        .map(|(fiber, packet)| Operation {
                            performing_fiber: *fiber,
                            kind: OperationKind::Send {
                                packet: packet.clone(),
                            },
                        })
                        .chain(pending_receives.iter().map(|fiber| Operation {
                            performing_fiber: *fiber,
                            kind: OperationKind::Receive,
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
        if let Some(fiber) = self.performing_fiber {
            write!(f, "{:?} ", fiber)?;
        }
        match &self.kind {
            OperationKind::Send { packet } => write!(f, "sending {:?}", packet),
            OperationKind::Receive => write!(f, "receiving"),
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
