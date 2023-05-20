use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    fmt::{self, Debug, Formatter},
    hash::Hash,
};

use candy_frontend::{
    hir::Id,
    id::{CountableId, IdGenerator},
};
use itertools::Itertools;
use rand::{seq::SliceRandom, thread_rng};

use crate::{
    channel::{Channel, ChannelId, Completer, Packet, Performer},
    context::{CombiningExecutionController, ExecutionController, RunLimitedNumberOfInstructions},
    fiber::{self, EndedReason, Fiber, FiberId, Panic, VmEnded},
    heap::{Data, Function, Heap, HeapObject, HirId, InlineObject, SendPort, Struct, Tag},
    lir::Lir,
    tracer::{FiberTracer, TracedFiberEnded, TracedFiberEndedReason, Tracer},
};
use extension_trait::extension_trait;
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationId(usize);
impl CountableId for OperationId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}
impl fmt::Debug for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "operation_{:x}", self.0)
    }
}

/// A VM represents a Candy program that thinks it's currently running. Because
/// VMs are first-class Rust structs, they enable other code to store "freezed"
/// programs and to remain in control about when and for how long code runs.
pub struct Vm<L: Borrow<Lir>, T: Tracer> {
    lir: L,
    fibers: HashMap<FiberId, FiberTree<T::ForFiber>>,

    channels: HashMap<ChannelId, ChannelLike>,
    pub completed_operations: HashMap<OperationId, CompletedOperation>,
    pub unreferenced_channels: HashSet<ChannelId>,

    operation_id_generator: IdGenerator<OperationId>,
    fiber_id_generator: IdGenerator<FiberId>,
    channel_id_generator: IdGenerator<ChannelId>,
}

pub enum FiberTree<T: FiberTracer> {
    /// This tree is currently focused on running a single fiber.
    Single(Single<T>),

    /// The fiber of this tree entered a `core.parallel` scope so that it's now
    /// paused and waits for the parallel scope to end. Instead of the main
    /// former single fiber, the tree now runs the function passed to
    /// `core.parallel` as well as any other spawned children.
    Parallel(Parallel<T>),

    Try(Try<T>),
}

/// Single fibers are the leaves of the fiber tree.
pub struct Single<T: FiberTracer> {
    pub fiber: Fiber<T>,
    parent: Option<FiberId>,
}

/// When a parallel section is entered, the fiber that started the section is
/// paused. Instead, the children of the parallel section are run. Initially,
/// there's only one child – the function given to the parallel builtin
/// function. Using the nursery parameter (a nursery can be thought of as a
/// pointer to a parallel section), you can also spawn other fibers. In contrast
/// to the first child, those children also have an explicit send port where the
/// function's result is sent to.
pub struct Parallel<T: FiberTracer> {
    pub paused_fiber: Single<T>,
    children: HashMap<FiberId, ChildKind>,
    return_value: Option<InlineObject>, // will later contain the body's return value
    nursery: ChannelId,
}
#[derive(Clone)]
enum ChildKind {
    InitialChild,
    SpawnedChild(ChannelId),
}

pub struct Try<T: FiberTracer> {
    pub paused_fiber: Single<T>,
    child: FiberId,
}

enum ChannelLike {
    Channel(Channel),
    Nursery(FiberId),
}

pub enum CompletedOperation {
    Sent,
    Received { packet: Packet },
}

#[derive(Clone, Debug)]
pub enum Status {
    CanRun,
    WaitingForOperations,
    Done,
    Panicked(Panic),
}

impl FiberId {
    pub fn root() -> Self {
        FiberId::from_usize(0)
    }
}

impl<L: Borrow<Lir>, T: Tracer> Vm<L, T> {
    pub fn uninitialized(lir: L) -> Self {
        Self {
            lir,
            fibers: HashMap::new(),
            channels: HashMap::new(),
            completed_operations: Default::default(),
            unreferenced_channels: Default::default(),
            operation_id_generator: Default::default(),
            channel_id_generator: Default::default(),
            fiber_id_generator: IdGenerator::start_at(FiberId::root().to_usize()),
        }
    }
    pub fn initialize_for_function(
        &mut self,
        heap: Heap,
        constant_mapping: FxHashMap<HeapObject, HeapObject>,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
        tracer: &mut T,
    ) {
        assert!(self.fibers.is_empty());

        let fiber = Fiber::for_function(
            heap,
            constant_mapping,
            function,
            arguments,
            responsible,
            tracer.root_fiber_created(),
        );
        self.fibers.insert(
            self.fiber_id_generator.generate(),
            FiberTree::Single(Single {
                fiber,
                parent: None,
            }),
        );
    }

    pub fn for_function(
        lir: L,
        heap: Heap,
        constant_mapping: FxHashMap<HeapObject, HeapObject>,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
        tracer: &mut T,
    ) -> Self {
        let mut this = Self::uninitialized(lir);
        this.initialize_for_function(
            heap,
            constant_mapping,
            function,
            arguments,
            responsible,
            tracer,
        );
        this
    }

    pub fn for_module(lir: L, tracer: &mut T) -> Self {
        let actual_lir = lir.borrow();
        let (heap, constant_mapping) = actual_lir.constant_heap.clone();

        let function = constant_mapping[&actual_lir.module_function]
            .try_into()
            .unwrap();
        let responsible = constant_mapping[&actual_lir.responsible_module]
            .try_into()
            .unwrap();

        Self::for_function(
            lir,
            heap,
            constant_mapping,
            function,
            &[],
            responsible,
            tracer,
        )
    }

    pub fn tear_down(mut self, tracer: &mut T) -> VmEnded {
        let tree = self.fibers.remove(&FiberId::root()).unwrap();
        let single = tree.into_single().unwrap();
        let mut ended = single.fiber.tear_down();
        tracer.root_fiber_ended(TracedFiberEnded {
            id: FiberId::root(),
            heap: &mut ended.heap,
            tracer: ended.tracer,
            reason: match &ended.reason {
                EndedReason::Finished(object) => TracedFiberEndedReason::Finished(*object),
                EndedReason::Panicked(reason) => TracedFiberEndedReason::Panicked(reason.clone()),
            },
        });
        VmEnded {
            heap: ended.heap,
            constant_mapping: ended.constant_mapping,
            reason: ended.reason,
        }
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
                fiber::Status::Panicked(panic) => Status::Panicked(panic.clone()),
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

    pub fn fibers(&self) -> &HashMap<FiberId, FiberTree<T::ForFiber>> {
        &self.fibers
    }
    pub fn fiber(&self, id: FiberId) -> Option<&FiberTree<T::ForFiber>> {
        self.fibers.get(&id)
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
    pub fn send(&mut self, channel: ChannelId, packet: Packet) -> OperationId {
        let operation_id = self.operation_id_generator.generate();
        self.send_to_channel(Performer::External(operation_id), channel, packet);
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

    pub fn run(&mut self, execution_controller: &mut impl ExecutionController, tracer: &mut T) {
        while self.can_run() && execution_controller.should_continue_running() {
            self.run_raw(
                &mut CombiningExecutionController::new(
                    execution_controller,
                    &mut RunLimitedNumberOfInstructions::new(100),
                ),
                tracer,
            );
        }
    }
    fn run_raw(&mut self, execution_controller: &mut impl ExecutionController, tracer: &mut T) {
        assert!(
            self.can_run(),
            "Called `Vm::run(…)` on a VM that is not ready to run."
        );

        // Choose a random fiber to run.
        let mut fiber_id = FiberId::root();
        let fiber = loop {
            match self.fibers.get_mut(&fiber_id).unwrap() {
                FiberTree::Single(Single { fiber, .. }) => break fiber,
                FiberTree::Parallel(Parallel { children, .. }) => {
                    let children_as_vec = children.iter().collect_vec();
                    let random_child = children_as_vec.choose(&mut thread_rng()).unwrap();
                    fiber_id = *random_child.0
                }
                FiberTree::Try(Try { child, .. }) => fiber_id = *child,
            }
        };
        if !matches!(fiber.status(), fiber::Status::Running) {
            return;
        }

        tracer.fiber_execution_started(fiber_id);
        fiber.run(self.lir.borrow(), execution_controller);

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
                self.send_to_channel(Performer::Fiber(fiber_id), channel, packet);
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
                    let id = self.fiber_id_generator.generate();

                    let (mut heap, mapping) = self.lir.borrow().constant_heap.clone();
                    let body = Data::from(body.clone_to_heap(&mut heap))
                        .try_into()
                        .unwrap();
                    let responsible =
                        HirId::create(&mut fiber.heap, Id::complicated_responsibility());
                    let nursery_send_port = SendPort::create(&mut heap, nursery_id);

                    let child_tracer = fiber.tracer.child_fiber_created(id);
                    self.fibers.insert(
                        id,
                        FiberTree::Single(Single {
                            fiber: Fiber::for_function(
                                heap,
                                mapping,
                                body,
                                &[nursery_send_port],
                                responsible,
                                child_tracer,
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
                    let id = self.fiber_id_generator.generate();
                    let (mut heap, mapping) = self.lir.borrow().constant_heap.clone();
                    let body = Data::from(body.clone_to_heap(&mut heap))
                        .try_into()
                        .unwrap();
                    let responsible =
                        HirId::create(&mut fiber.heap, Id::complicated_responsibility());

                    let child_tracer = fiber.tracer.child_fiber_created(id);
                    self.fibers.insert(
                        id,
                        FiberTree::Single(Single {
                            fiber: Fiber::for_function(
                                heap,
                                mapping,
                                body,
                                &[],
                                responsible,
                                child_tracer,
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
            fiber::Status::Done | fiber::Status::Panicked { .. } => true,
        };

        if is_finished && fiber_id != FiberId::root() {
            let single = self
                .fibers
                .remove(&fiber_id)
                .unwrap()
                .into_single()
                .unwrap();
            let mut ended = single.fiber.tear_down();
            let parent = single
                .parent
                .expect("We already checked we're not the root fiber.");

            match self.fibers.get_mut(&parent).unwrap() {
                FiberTree::Single(_) => unreachable!("Single fibers can't have children."),
                FiberTree::Parallel(parallel) => {
                    let child = parallel.children.remove(&fiber_id).unwrap();

                    let parallel_result = match &ended.reason {
                        EndedReason::Finished(return_value) => {
                            let is_finished = parallel.children.is_empty();
                            match child {
                                ChildKind::InitialChild => {
                                    parallel.return_value = Some(*return_value)
                                }
                                ChildKind::SpawnedChild(return_channel) => {
                                    self.send_to_channel(
                                        Performer::Nursery,
                                        return_channel,
                                        (*return_value).into(),
                                    );
                                    return_value.drop(&mut ended.heap);
                                }
                            }

                            if is_finished {
                                Some(Ok(()))
                            } else {
                                None
                            }
                        }
                        EndedReason::Panicked(panic) => Some(Err(panic.to_owned())),
                    };

                    self.fibers
                        .get_mut(&parent)
                        .unwrap()
                        .as_parallel_mut()
                        .unwrap()
                        .paused_fiber
                        .fiber
                        .adopt_finished_child(fiber_id, ended);

                    if let Some(parallel_result) = parallel_result {
                        self.finish_parallel(parent, parallel_result)
                    }
                }
                FiberTree::Try(Try { child, .. }) => {
                    let child_id = *child;
                    self.fibers.replace(parent, |tree| {
                        let mut paused_fiber = tree.into_try().unwrap().paused_fiber;

                        let reason = ended.reason.clone();
                        paused_fiber.fiber.adopt_finished_child(child_id, ended);
                        paused_fiber.fiber.complete_try(&reason);
                        FiberTree::Single(paused_fiber)
                    });
                }
            }
        }

        let all_channels = self.channels.keys().copied().collect::<HashSet<_>>();
        let mut known_channels = HashSet::new();
        for fiber in self.fibers.values() {
            if let Some(single) = fiber.as_single() {
                known_channels.extend(single.fiber.heap.known_channels());
            }
        }
        // Because we don't track yet which channels have leaked to the outside
        // world, any channel may be re-sent into the VM from the outside even
        // after no fibers remember it. Rather than removing it directly, we
        // communicate to the outside that no fiber references it anymore. If
        // the outside doesn't intend to re-use the channel, it should call
        // `free_channel`.
        self.unreferenced_channels = all_channels
            .difference(&known_channels)
            .filter(|channel| {
                // Note that nurseries are automatically removed when their
                // parallel scope is exited.
                matches!(self.channels.get(channel).unwrap(), ChannelLike::Channel(_))
            })
            .copied()
            .collect();
    }
    fn finish_parallel(&mut self, parallel_id: FiberId, result: Result<(), Panic>) {
        let parallel = self
            .fibers
            .get_mut(&parallel_id)
            .unwrap()
            .as_parallel_mut()
            .unwrap();

        for child_id in parallel.children.clone().into_keys() {
            self.cancel(parallel_id, child_id);
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
    }
    fn cancel(&mut self, parent_id: FiberId, fiber_id: FiberId) {
        let fiber_tree = self.fibers.remove(&fiber_id).unwrap();
        match &fiber_tree {
            FiberTree::Single(_) => {}
            FiberTree::Parallel(Parallel {
                children, nursery, ..
            }) => {
                self.channels.remove(nursery).unwrap().to_nursery().unwrap();

                for child_fiber in children.keys() {
                    self.cancel(fiber_id, *child_fiber);
                }
            }
            FiberTree::Try(Try { child, .. }) => self.cancel(fiber_id, *child),
        }

        let parent = self.fibers.get_mut(&parent_id).unwrap().fiber_mut();
        parent.tracer.child_fiber_ended(TracedFiberEnded {
            id: fiber_id,
            heap: &mut parent.heap,
            tracer: fiber_tree.into_fiber().tracer,
            reason: TracedFiberEndedReason::Canceled,
        });
    }

    fn send_to_channel(&mut self, performer: Performer, channel_id: ChannelId, packet: Packet) {
        let channel = match self.channels.get_mut(&channel_id) {
            Some(channel) => channel,
            None => {
                // The channel was a nursery that died.
                if let Performer::Fiber(fiber) = performer {
                    let tree = self.fibers.get_mut(&fiber).unwrap();
                    tree.as_single_mut()
                        .unwrap()
                        .fiber
                        .panic(Panic::new_without_responsible(
                            "The nursery is already dead because the parallel section ended."
                                .to_string(),
                        ));
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
                    Some((_packet_heap, function_to_spawn, return_channel)) => {
                        let (mut heap, constant_mapping) = self.lir.borrow().constant_heap.clone();
                        let function_to_spawn: Function = function_to_spawn
                            .clone_to_heap(&mut heap)
                            .try_into()
                            .unwrap();
                        let responsible =
                            HirId::create(&mut heap, Id::complicated_responsibility());
                        let child_id = self.fiber_id_generator.generate();

                        let parent = self
                            .fibers
                            .get_mut(&parent_id)
                            .unwrap()
                            .as_parallel_mut()
                            .unwrap();
                        let child_tracer = parent
                            .paused_fiber
                            .fiber
                            .tracer
                            .child_fiber_created(child_id);
                        self.fibers.insert(
                            child_id,
                            FiberTree::Single(Single {
                                fiber: Fiber::for_function(
                                    heap,
                                    constant_mapping,
                                    function_to_spawn,
                                    &[],
                                    responsible,
                                    child_tracer,
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
                    }
                    None => self.finish_parallel(
                        parent_id,
                        Err(Panic::new_without_responsible(
                            "A nursery received an invalid message.".to_string(),
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
    fn parse_spawn_packet(packet: Packet) -> Option<(Heap, Function, ChannelId)> {
        let Packet { mut heap, object } = packet;
        let arguments: Struct = object.try_into().ok()?;

        let function_tag = Tag::create_from_str(&mut heap, "Function", None);
        let function: Function = arguments.get(**function_tag)?.try_into().ok()?;
        if function.argument_count() > 0 {
            return None;
        }

        let return_channel_tag = Tag::create_from_str(&mut heap, "ReturnChannel", None);
        let return_channel: SendPort = arguments.get(**return_channel_tag)?.try_into().ok()?;

        Some((heap, function, return_channel.channel_id()))
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
            ChannelLike::Nursery { .. } => unreachable!("Nurseries are only sent stuff."),
        }
    }
}

struct InternalCompleter<'a, T: FiberTracer> {
    fibers: &'a mut HashMap<FiberId, FiberTree<T>>,
    completed_operations: &'a mut HashMap<OperationId, CompletedOperation>,
}
impl<'a, T: FiberTracer> Completer for InternalCompleter<'a, T> {
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
impl<T: FiberTracer> FiberTree<T> {
    fn into_fiber(self) -> Fiber<T> {
        match self {
            FiberTree::Single(single) => single.fiber,
            FiberTree::Parallel(parallel) => parallel.paused_fiber.fiber,
            FiberTree::Try(try_) => try_.paused_fiber.fiber,
        }
    }
    pub fn fiber_ref(&self) -> &Fiber<T> {
        match self {
            FiberTree::Single(single) => &single.fiber,
            FiberTree::Parallel(parallel) => &parallel.paused_fiber.fiber,
            FiberTree::Try(try_) => &try_.paused_fiber.fiber,
        }
    }
    pub fn fiber_mut(&mut self) -> &mut Fiber<T> {
        match self {
            FiberTree::Single(single) => &mut single.fiber,
            FiberTree::Parallel(parallel) => &mut parallel.paused_fiber.fiber,
            FiberTree::Try(try_) => &mut try_.paused_fiber.fiber,
        }
    }

    // TODO: Use macros to generate these.
    fn into_single(self) -> Option<Single<T>> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }
    fn as_single(&self) -> Option<&Single<T>> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }
    fn as_single_mut(&mut self) -> Option<&mut Single<T>> {
        match self {
            FiberTree::Single(single) => Some(single),
            _ => None,
        }
    }

    fn into_parallel(self) -> Option<Parallel<T>> {
        match self {
            FiberTree::Parallel(parallel) => Some(parallel),
            _ => None,
        }
    }
    fn as_parallel_mut(&mut self) -> Option<&mut Parallel<T>> {
        match self {
            FiberTree::Parallel(parallel) => Some(parallel),
            _ => None,
        }
    }

    fn into_try(self) -> Option<Try<T>> {
        match self {
            FiberTree::Try(try_) => Some(try_),
            _ => None,
        }
    }
}

impl<L: Borrow<Lir>, T: Tracer> Debug for Vm<L, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vm")
            .field("fibers", &self.fibers)
            .field("channels", &self.channels)
            .finish()
    }
}
impl<T: FiberTracer> Debug for FiberTree<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
impl Debug for ChildKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ChildKind::InitialChild => write!(f, "is initial child"),
            ChildKind::SpawnedChild(return_channel) => write!(f, "returns to {:?}", return_channel),
        }
    }
}
impl Debug for ChannelLike {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(channel) => channel.fmt(f),
            Self::Nursery(fiber) => f.debug_tuple("Nursery").field(fiber).finish(),
        }
    }
}

#[extension_trait]
impl<K: Eq + Hash, V> ReplaceHashMapValue<K, V> for HashMap<K, V> {
    fn replace<F: FnOnce(V) -> V>(&mut self, key: K, replacer: F) {
        let value = self.remove(&key).unwrap();
        let value = replacer(value);
        self.insert(key, value);
    }
}
