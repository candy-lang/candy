use candy_vm::{
    channel::{ChannelId, Packet},
    heap::{Heap, SendPort, Text},
    lir::Lir,
    tracer::DummyTracer,
    vm::{CompletedOperation, OperationId, Vm},
};
use std::{
    borrow::Borrow,
    io::{self, BufRead, Write},
};

pub struct StdinService {
    pub channel: ChannelId,
    current_receive: OperationId,
}
impl StdinService {
    pub fn new<L: Borrow<Lir>>(vm: &mut Vm<L>) -> Self {
        let channel = vm.create_channel(0);
        let current_receive = vm.receive(channel);
        Self {
            channel,
            current_receive,
        }
    }

    pub fn run<L: Borrow<Lir>>(&mut self, vm: &mut Vm<L>) {
        while let Some(CompletedOperation::Received { packet }) =
            vm.completed_operations.remove(&self.current_receive)
        {
            let request: SendPort = packet
                .object
                .try_into()
                .expect("Expected a send port to be sent to stdin.");
            print!(">> ");
            io::stdout().flush().unwrap();
            let input = {
                let stdin = io::stdin();
                stdin.lock().lines().next().unwrap().unwrap()
            };
            let packet = {
                let mut heap = Heap::default();
                let object = Text::create(&mut heap, &input).into();
                Packet { heap, object }
            };
            vm.send(&mut DummyTracer, request.channel_id(), packet);

            // Receive the next request
            self.current_receive = vm.receive(self.channel);
        }
    }
}