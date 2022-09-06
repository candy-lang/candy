mod builtin_functions;
mod channel;
pub mod context;
mod fiber;
mod heap;
pub mod tracer;
mod use_module;

use std::{marker::PhantomData, collections::{HashMap, VecDeque}, fmt};
pub use fiber::{Fiber, TearDownResult};
pub use heap::{Closure, Heap, Object, Pointer};
use rand::seq::SliceRandom;
use tracing::{info, warn};
use self::{heap::ChannelId, channel::{ChannelBuf, Packet}, context::Context, tracer::Tracer};

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
/// 






#[derive(Clone)]
pub struct Vm {
    fibers: HashMap<FiberId, FiberTree>,
    root_fiber: FiberId,
    
    channels: HashMap<ChannelId, Channel>,
    pub external_operations: HashMap<ChannelId, Vec<Operation>>,

    fiber_id_generator: IdGenerator<FiberId>,
    channel_id_generator: IdGenerator<ChannelId>,
}

type FiberId = usize;
type OperationId = usize;

#[derive(Clone, Debug)]
enum Channel {
    Internal {
        buffer: ChannelBuf,
        pending_operations: VecDeque<Operation>,
    },
    External(ChannelId),
    Nursery {
        children: Vec<Child>,
    }
}
#[derive(Clone, Debug)]
struct Child {
    fiber: FiberId,
    return_value_channel: ChannelId,
}

#[derive(Clone)]
pub struct Operation {
    performing_fiber: FiberId,
    kind: OperationKind,
}
#[derive(Clone, Debug)]
pub enum OperationKind {
    Send { packet: Packet },
    Receive,
}

#[derive(Clone)]
enum FiberTree {
    /// This tree is currently focused on running a single fiber.
    SingleFiber(Fiber),

    /// The fiber of this tree entered a `core.parallel` scope so that it's now
    /// paused and waits for the parallel scope to end. Instead of the main
    /// former single fiber, the tree now runs the closure passed to
    /// `core.parallel` as well as any other spawned children.
    ParallelSection {
        paused_main_fiber: Fiber, // Should have Status::InParallelSection.
        nursery: ChannelId,
    },
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    WaitingForOperations,
    Done,
    Panicked { reason: String },
}




impl Vm {
    fn new_with_fiber(mut fiber: Fiber) -> Self {
        let fiber = FiberTree::SingleFiber(fiber);
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
        let fiber = self.fibers.remove(&self.root_fiber).unwrap();
        let fiber = match fiber {
            FiberTree::SingleFiber(fiber) => fiber,
            FiberTree::ParallelSection { .. } => unreachable!(),
        };
        fiber.tear_down()
    }

    pub fn status(&self) -> Status {
        self.status_of(self.root_fiber)
    }
    fn status_of(&self, fiber: FiberId) -> Status {
        match &self.fibers[&fiber] {
            FiberTree::SingleFiber(fiber) => match &fiber.status {
                fiber::Status::Running => Status::Running,
                fiber::Status::Sending { .. } |
                fiber::Status::Receiving { .. } => Status::WaitingForOperations,
                fiber::Status::CreatingChannel { .. } |
                fiber::Status::InParallelScope { .. } => unreachable!(),
                fiber::Status::Done => Status::Done,
                fiber::Status::Panicked { reason } => Status::Panicked { reason: reason.clone() },
            },
            FiberTree::ParallelSection { nursery, .. } => {
                let children = match &self.channels[nursery] {
                    Channel::Nursery { children } => children,
                    _ => unreachable!(),
                };
                for child in children {
                    return match self.status_of(child.fiber) {
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

    pub fn fiber(&self) -> &Fiber { // TODO: Remove before merging the PR
        todo!()
    }
    pub fn cloned_tracer(&self) -> Tracer {
        self.fiber().tracer.clone()
    }

    fn complete_send(&mut self, fiber: FiberId) {
        let fiber = self.fibers.get_mut(&fiber).unwrap();
        let fiber = match fiber {
            FiberTree::SingleFiber(fiber) => fiber,
            FiberTree::ParallelSection { .. } => unreachable!(),
        };
        fiber.complete_send();
    }
    fn complete_receive(&mut self, fiber: FiberId, packet: Packet) {
        let fiber = self.fibers.get_mut(&fiber).unwrap();
        let fiber = match fiber {
            FiberTree::SingleFiber(fiber) => fiber,
            FiberTree::ParallelSection { .. } => unreachable!(),
        };
        fiber.complete_receive(packet);
    }

    pub fn run<C: Context>(&mut self, context: &mut C) {
        assert!(
            self.is_running(),
            "Called Vm::run on a VM that is not ready to run."
        );

        let mut fiber_id = self.root_fiber;
        let fiber = loop {
            match self.fibers.get_mut(&fiber_id).unwrap() {
                FiberTree::SingleFiber(fiber) => break fiber,
                FiberTree::ParallelSection { nursery, .. } => {
                    let children = match &self.channels[&nursery] {
                        Channel::Nursery { children } => children,
                        _ => unreachable!(),
                    };
                    fiber_id = children
                        .choose(&mut rand::thread_rng()).unwrap().fiber;
                },
            }
        };

        fiber.run(context);

        match fiber.status() {
            fiber::Status::Running => {},
            fiber::Status::CreatingChannel { capacity } => {
                let channel_id = self.channel_id_generator.generate();
                self.channels.insert(channel_id, Channel::Internal { buffer: ChannelBuf::new(capacity), pending_operations: Default::default() });
                fiber.complete_channel_create(channel_id);
            },
            fiber::Status::Sending { channel, packet } => 
                self.send_to_channel(fiber_id, channel, packet),
            fiber::Status::Receiving { channel } => 
                self.receive_from_channel(fiber_id, channel),
            fiber::Status::InParallelScope { body, return_channel } => {
                let nursery_id = self.channel_id_generator.generate();

                let child_id = {
                    let mut heap = Heap::default();
                    let body = fiber.heap.clone_single_to_other_heap(&mut heap, body);
                    let nursery_send_port = heap.create_send_port(nursery_id);
                    let id = self.fiber_id_generator.generate();
                    self.fibers.insert(id, FiberTree::SingleFiber(Fiber::new_for_running_closure(heap, body, &[nursery_send_port])));
                    id
                };

                let nursery_id = {
                    let id = self.fiber_id_generator.generate();
                    // TODO: Make it so that the initial fiber doesn't need a return channel.
                    let children = vec![Child {
                        fiber: child_id,
                        return_value_channel: return_channel,
                    }];
                    self.channels.insert(id, Channel::Nursery { children });
                    id
                };

                let paused_main_fiber = match self.fibers.remove(&fiber_id).unwrap() {
                    FiberTree::SingleFiber(fiber) => fiber,
                    _ => unreachable!(),
                };
                self.fibers.insert(fiber_id, FiberTree::ParallelSection { paused_main_fiber, nursery: nursery_id });

                // self.fibers.entry(fiber_id).and_modify(|fiber_tree| {
                //     let paused_main_fiber = match original {}
                //     FiberTree::ParallelSection { paused_main_fiber, nursery: nursery_id }
                // });
            },
            fiber::Status::Done => {
                info!("A fiber is done.");
            },
            fiber::Status::Panicked { reason } => {
                warn!("A fiber panicked because {reason}.");
            },
        }
    }

    fn send_to_channel(&mut self, performing_fiber: FiberId, channel: ChannelId, packet: Packet) {
        match self.channels.get_mut(&channel).unwrap() {
            Channel::Internal { buffer, pending_operations } => {
                // TODO: Make multithreaded-working.
                if buffer.is_full() {
                    pending_operations.push_back(Operation {
                        performing_fiber,
                        kind: OperationKind::Send { packet },
                    });
                } else {
                    buffer.send(packet);
                    self.complete_send(performing_fiber);
                }
            },
            Channel::External(id) => {
                let id = *id;
                self.push_external_operation(id, Operation {
                    performing_fiber,
                    kind: OperationKind::Send { packet },
                })
            },
            Channel::Nursery { children } => {
                todo!("Stuff is being sent to nursery.");
            },
        }
    }

    fn receive_from_channel(&mut self, performing_fiber: FiberId, channel: ChannelId) {
        match self.channels.get_mut(&channel).unwrap() {
            Channel::Internal { buffer, pending_operations } => {
                if buffer.is_empty() {
                    pending_operations.push_back(Operation {
                        performing_fiber,
                        kind: OperationKind::Receive,
                    });
                } else {
                    let packet = buffer.receive().unwrap();
                    self.complete_receive(performing_fiber, packet);
                }
            },
            Channel::External(id) => {
                let id = *id;
                self.push_external_operation(id, Operation {
                    performing_fiber,
                    kind: OperationKind::Receive,
                });
            },
            Channel::Nursery { .. } => unreachable!("nurseries are only sent stuff"),
        }
    }

    fn push_external_operation(&mut self, channel: ChannelId, operation: Operation) {
        self.external_operations.entry(channel).or_default().push(operation);
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

impl fmt::Debug for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            OperationKind::Send { packet } => write!(f, "{} sending {:?}", self.performing_fiber, packet),
            OperationKind::Receive => write!(f, "{} receiving", self.performing_fiber),
        }
    }
}
impl fmt::Debug for FiberTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleFiber(arg0) => f.debug_tuple("SingleFiber").finish(),
            Self::ParallelSection { paused_main_fiber, nursery } => f.debug_struct("ParallelSection").field("nursery", nursery).finish(),
        }
    }
}
impl fmt::Debug for Vm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vm").field("fibers", &self.fibers).field("channels", &self.channels).field("external_operations", &self.external_operations).finish()
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
