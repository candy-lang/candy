use candy_vm::{
    channel::ChannelId,
    heap::Data,
    vm::{CompletedOperation, OperationId, Vm},
};
use tracing::info;

pub struct StdoutService {
    pub channel: ChannelId,
    current_receive: OperationId,
}
impl StdoutService {
    pub fn new(vm: &mut Vm) -> Self {
        let channel = vm.create_channel(0);
        let current_receive = vm.receive(channel);
        Self {
            channel,
            current_receive,
        }
    }

    pub fn run(&mut self, vm: &mut Vm) {
        while let Some(CompletedOperation::Received { packet }) =
            vm.completed_operations.remove(&self.current_receive)
        {
            match packet.object.into() {
                Data::Text(text) => println!("{}", text.get()),
                _ => info!("Non-text value sent to stdout: {packet:?}"),
            }
            self.current_receive = vm.receive(self.channel);
        }
    }
}
