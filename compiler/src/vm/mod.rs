mod builtin_functions;
mod channel;
pub mod context;
mod fiber;
mod heap;
mod ids;
pub mod tracer;
mod use_module;

pub use self::{
    channel::Packet,
    fiber::{ExecutionResult, Fiber},
    heap::{Closure, Data, Heap, Object, Pointer, SendPort, Struct},
    ids::{ChannelId, FiberId, OperationId},
    tracer::{full::FullTracer, Tracer},
};
use self::{
    channel::{Channel, Completer, Performer},
    context::{
        CombiningExecutionController, ExecutionController, RunLimitedNumberOfInstructions,
        UseProvider,
    },
};
use crate::{
    compiler::hir::Id,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
};

/// A VM represents a Candy program that thinks it's currently running. Because
/// VMs are first-class Rust structs, they enable other code to store "freezed"
/// programs and to remain in control about when and for how long code runs.
#[derive(Clone)]
pub struct Vm {
    fibers: HashMap<FiberId, FiberTree>,

    channels: HashMap<ChannelId, ChannelLike>,
    pub completed_operations: HashMap<OperationId, CompletedOperation>,
    pub unreferenced_channels: HashSet<ChannelId>,

    operation_id_generator: IdGenerator<OperationId>,
    fiber_id_generator: IdGenerator<FiberId>,
    channel_id_generator: IdGenerator<ChannelId>,
}

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

/// Single fibers are the leaves of the fiber tree.
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
enum ChannelLike {
    Channel(Channel),
    Nursery(FiberId),
}

#[derive(Clone)]
pub enum CompletedOperation {
    Sent,
    Received { packet: Packet },
}

#[derive(Clone, Debug)]
pub enum Status {
    CanRun,
    WaitingForOperations,
    Done,
    Panicked { reason: String, responsible: Id },
}

impl FiberId {
    pub fn root() -> Self {
        FiberId::from_usize(0)
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            fibers: HashMap::new(),
            channels: HashMap::new(),
            completed_operations: Default::default(),
            unreferenced_channels: Default::default(),
            operation_id_generator: Default::default(),
            channel_id_generator: Default::default(),
            fiber_id_generator: IdGenerator::start_at(FiberId::root().to_usize() + 1),
        }
    }

    fn set_up_with_fiber(&mut self, fiber: Fiber) {
        assert!(
            !self.fibers.contains_key(&FiberId::root()),
            "already set up"
        );
        self.fibers.insert(
            FiberId::root(),
            FiberTree::Single(Single {
                fiber,
                parent: None,
            }),
        );
    }
    pub fn set_up_for_running_closure(
        &mut self,
        heap: Heap,
        closure: Pointer,
        arguments: Vec<Pointer>,
        responsible: Id,
    ) {
        self.set_up_with_fiber(Fiber::new_for_running_closure(
            heap,
            closure,
            arguments,
            responsible,
        ));
    }
    pub fn set_up_for_running_module_closure(&mut self, module: Module, closure: Closure) {
        self.set_up_with_fiber(Fiber::new_for_running_module_closure(module, closure))
    }

    pub fn tear_down(mut self) -> ExecutionResult {
        let tree = self.fibers.remove(&FiberId::root()).unwrap();
        let single = tree.into_single().unwrap();
        single.fiber.tear_down()
    }

    pub fn status(&self) -> Status {
        self.status_of(FiberId::root())
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
                fiber::Status::Panicked {
                    reason,
                    responsible,
                } => Status::Panicked {
                    reason: reason.clone(),
                    responsible: responsible.clone(),
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

    /// Can be called at any time from outside the VM to create a channel that
    /// can be used to communicate with the outside world.
    pub fn create_channel(&mut self, capacity: usize) -> ChannelId {
        let id = self.channel_id_generator.generate();
        self.channels
            .insert(id, ChannelLike::Channel(Channel::new(capacity)));
        id
    }

    // This will be used as soon as the outside world tries to send something
    // into the VM.
    #[allow(dead_code)]
    pub fn send<T: Tracer>(
        &mut self,
        tracer: &mut T,
        channel: ChannelId,
        packet: Packet,
    ) -> OperationId {
        let operation_id = self.operation_id_generator.generate();
        self.send_to_channel(tracer, Performer::External(operation_id), channel, packet);
        operation_id
    }

    pub fn receive(&mut self, channel: ChannelId) -> OperationId {
        let operation_id = self.operation_id_generator.generate();
        self.receive_from_channel(Performer::External(operation_id), channel);
        operation_id
    }

    pub fn free_unreferenced_channels(&mut self) {
        for channel in self.unreferenced_channels.iter().copied().collect_vec() {
            self.free_channel(channel);
        }
    }

    /// May only be called if the channel is in the `unreferenced_channels`.
    pub fn free_channel(&mut self, channel: ChannelId) {
        assert!(self.unreferenced_channels.contains(&channel));
        self.channels.remove(&channel);
        self.unreferenced_channels.remove(&channel);
    }

    pub fn run<U: UseProvider, E: ExecutionController, T: Tracer>(
        &mut self,
        use_provider: &U,
        execution_controller: &mut E,
        tracer: &mut T,
    ) {
        while self.can_run() && execution_controller.should_continue_running() {
            self.run_raw(
                use_provider,
                &mut CombiningExecutionController::new(
                    execution_controller,
                    &mut RunLimitedNumberOfInstructions::new(100),
                ),
                tracer,
            );
        }
    }
    fn run_raw<U: UseProvider, E: ExecutionController, T: Tracer>(
        &mut self,
        use_provider: &U,
        execution_controller: &mut E,
        tracer: &mut T,
    ) {
        assert!(
            self.can_run(),
            "Called Vm::run on a VM that is not ready to run."
        );

        // Choose a random fiber to run.
        let mut fiber_id = FiberId::root();
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

        tracer.fiber_execution_started(fiber_id);
        fiber.run(
            use_provider,
            execution_controller,
            &mut tracer.for_fiber(fiber_id),
        );
        tracer.fiber_execution_ended(fiber_id);

        let is_finished = match fiber.status() {
            fiber::Status::Running => false,
            fiber::Status::CreatingChannel { capacity } => {
                let channel_id = self.channel_id_generator.generate();
                self.channels
                    .insert(channel_id, ChannelLike::Channel(Channel::new(capacity)));
                fiber.complete_channel_create(channel_id);
                tracer.channel_created(channel_id);
                false
            }
            fiber::Status::Sending { channel, packet } => {
                self.send_to_channel(tracer, Performer::Fiber(fiber_id), channel, packet);
                false
            }
            fiber::Status::Receiving { channel } => {
                self.receive_from_channel(Performer::Fiber(fiber_id), channel);
                false
            }
            fiber::Status::InParallelScope { body } => {
                let nursery_id = self.channel_id_generator.generate();
                self.channels
                    .insert(nursery_id, ChannelLike::Nursery(fiber_id));

                let first_child_id = {
                    let mut heap = Heap::default();
                    let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                    let nursery_send_port = heap.create_send_port(nursery_id);
                    let id = self.fiber_id_generator.generate();
                    self.fibers.insert(
                        id,
                        FiberTree::Single(Single {
                            fiber: Fiber::new_for_running_closure(
                                heap,
                                body,
                                vec![nursery_send_port],
                                Id::complicated_responsibility(),
                            ),
                            parent: Some(fiber_id),
                        }),
                    );
                    id
                };

                self.fibers.replace(fiber_id, |tree| {
                    let single = tree.into_single().unwrap();
                    FiberTree::Parallel(Parallel {
                        paused_fiber: single,
                        children: HashMap::from([(first_child_id, ChildKind::InitialChild)]),
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
                            fiber: Fiber::new_for_running_closure(
                                heap,
                                body,
                                vec![],
                                Id::complicated_responsibility(),
                            ),
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
                tracer.fiber_done(fiber_id);
                true
            }
            fiber::Status::Panicked { .. } => {
                tracer.fiber_panicked(fiber_id, None);
                true
            }
        };

        if is_finished && fiber_id != FiberId::root() {
            let single = self
                .fibers
                .remove(&fiber_id)
                .unwrap()
                .into_single()
                .unwrap();
            let result = single.fiber.tear_down();
            let parent = single
                .parent
                .expect("we already checked we're not the root fiber");

            match self.fibers.get_mut(&parent).unwrap() {
                FiberTree::Single(_) => unreachable!("single fibers can't have children"),
                FiberTree::Parallel(parallel) => {
                    let child = parallel.children.remove(&fiber_id).unwrap();

                    match result {
                        ExecutionResult::Finished(return_value) => {
                            let is_finished = parallel.children.is_empty();
                            match child {
                                ChildKind::InitialChild => {
                                    parallel.return_value = Some(return_value)
                                }
                                ChildKind::SpawnedChild(return_channel) => self.send_to_channel(
                                    tracer,
                                    Performer::Nursery,
                                    return_channel,
                                    return_value,
                                ),
                            }

                            if is_finished {
                                self.finish_parallel(
                                    tracer,
                                    parent,
                                    Performer::Fiber(fiber_id),
                                    Ok(()),
                                )
                            }
                        }
                        ExecutionResult::Panicked {
                            reason,
                            responsible,
                        } => self.finish_parallel(
                            tracer,
                            parent,
                            Performer::Fiber(fiber_id),
                            Err((reason, responsible)),
                        ),
                    }
                }
                FiberTree::Try(Try { .. }) => {
                    self.fibers.replace(parent, |tree| {
                        let mut paused_fiber = tree.into_try().unwrap().paused_fiber;
                        paused_fiber.fiber.complete_try(result);
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
        // Because we don't track yet which channels have leaked to the outside
        // world, any channel may be re-sent into the VM from the outside even
        // after no fibers remember it. Rather than removing it directly, we
        // communicate to the outside that no fiber references it anymore. If
        // the outside doesn't intend to re-use the channel, it should call
        // `free_channel`.
        let unreferenced_channels = all_channels
            .difference(&known_channels)
            .filter(|channel| {
                // Note that nurseries are automatically removed when their
                // parallel scope is exited.
                matches!(self.channels.get(channel).unwrap(), ChannelLike::Channel(_))
            })
            .copied()
            .collect();
        self.unreferenced_channels = unreferenced_channels;
    }
    fn finish_parallel<T: Tracer>(
        &mut self,
        tracer: &mut T,
        parallel_id: FiberId,
        cause: Performer,
        result: Result<(), (String, Id)>,
    ) {
        let parallel = self
            .fibers
            .get_mut(&parallel_id)
            .unwrap()
            .as_parallel_mut()
            .unwrap();

        for child_id in parallel.children.clone().into_keys() {
            self.cancel(tracer, child_id);
        }

        self.fibers.replace(parallel_id, |tree| {
            let Parallel {
                mut paused_fiber,
                nursery,
                return_value,
                ..
            } = tree.into_parallel().unwrap();
            self.channels.remove(&nursery).unwrap();
            paused_fiber
                .fiber
                .complete_parallel_scope(result.map(|_| return_value.unwrap()));
            FiberTree::Single(paused_fiber)
        });
        tracer.fiber_panicked(
            parallel_id,
            match cause {
                Performer::Fiber(fiber) => Some(fiber),
                _ => None,
            },
        );
    }
    fn cancel(&mut self, tracer: &mut dyn Tracer, fiber: FiberId) {
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
                    self.cancel(tracer, *child_fiber);
                }
            }
            FiberTree::Try(Try { child, .. }) => self.cancel(tracer, child),
        }
        tracer.fiber_canceled(fiber);
    }

    fn send_to_channel<T: Tracer>(
        &mut self,
        tracer: &mut T,
        performer: Performer,
        channel_id: ChannelId,
        packet: Packet,
    ) {
        let channel = match self.channels.get_mut(&channel_id) {
            Some(channel) => channel,
            None => {
                // The channel was a nursery that died.
                if let Performer::Fiber(fiber) = performer {
                    let tree = self.fibers.get_mut(&fiber).unwrap();
                    tree.as_single_mut().unwrap().fiber.panic(
                        "The nursery is already dead because the parallel section ended."
                            .to_string(),
                        Id::complicated_responsibility(),
                    );
                }
                return;
            }
        };
        match channel {
            ChannelLike::Channel(channel) => {
                let mut completer = InternalCompleter {
                    fibers: &mut self.fibers,
                    completed_operations: &mut self.completed_operations,
                };
                channel.send(&mut completer, performer, packet);
            }
            ChannelLike::Nursery(parent_id) => {
                let parent_id = *parent_id;

                match Self::parse_spawn_packet(packet) {
                    Some((heap, closure_to_spawn, return_channel)) => {
                        let child_id = self.fiber_id_generator.generate();
                        self.fibers.insert(
                            child_id,
                            FiberTree::Single(Single {
                                fiber: Fiber::new_for_running_closure(
                                    heap,
                                    closure_to_spawn,
                                    vec![],
                                    Id::complicated_responsibility(),
                                ),
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

                        tracer.fiber_created(child_id);
                    }
                    None => self.finish_parallel(
                        tracer,
                        parent_id,
                        performer.clone(),
                        Err((
                            "a nursery received an invalid message".to_string(),
                            Id::complicated_responsibility(),
                        )),
                    ),
                }

                InternalCompleter {
                    fibers: &mut self.fibers,
                    completed_operations: &mut self.completed_operations,
                }
                .complete_send(performer);
            }
        }
    }
    fn parse_spawn_packet(packet: Packet) -> Option<(Heap, Pointer, ChannelId)> {
        let Packet { mut heap, address } = packet;
        let arguments: Struct = heap.get(address).data.clone().try_into().ok()?;

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

    fn receive_from_channel(&mut self, performer: Performer, channel: ChannelId) {
        let mut completer = InternalCompleter {
            fibers: &mut self.fibers,
            completed_operations: &mut self.completed_operations,
        };
        match self.channels.get_mut(&channel).unwrap() {
            ChannelLike::Channel(channel) => {
                channel.receive(&mut completer, performer);
            }
            ChannelLike::Nursery { .. } => unreachable!("nurseries are only sent stuff"),
        }
    }
}

struct InternalCompleter<'a> {
    fibers: &'a mut HashMap<FiberId, FiberTree>,
    completed_operations: &'a mut HashMap<OperationId, CompletedOperation>,
}
impl<'a> Completer for InternalCompleter<'a> {
    fn complete_send(&mut self, performer: Performer) {
        match performer {
            Performer::Fiber(fiber) => {
                let tree = self.fibers.get_mut(&fiber).unwrap();
                tree.as_single_mut().unwrap().fiber.complete_send();
            }
            Performer::Nursery => {}
            Performer::External(id) => {
                self.completed_operations
                    .insert(id, CompletedOperation::Sent);
            }
        }
    }

    fn complete_receive(&mut self, performer: Performer, packet: Packet) {
        match performer {
            Performer::Fiber(fiber) => {
                let tree = self.fibers.get_mut(&fiber).unwrap();
                tree.as_single_mut().unwrap().fiber.complete_receive(packet);
            }
            Performer::Nursery => {}
            Performer::External(id) => {
                self.completed_operations
                    .insert(id, CompletedOperation::Received { packet });
            }
        }
    }
}

impl ChannelLike {
    fn to_nursery(&self) -> Option<FiberId> {
        match self {
            ChannelLike::Nursery(fiber) => Some(*fiber),
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
            .finish()
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
impl fmt::Debug for ChannelLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(channel) => channel.fmt(f),
            Self::Nursery(fiber) => f.debug_tuple("Nursery").field(fiber).finish(),
        }
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
