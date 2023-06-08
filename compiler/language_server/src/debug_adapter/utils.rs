use candy_frontend::id::CountableId;
use candy_vm::fiber::FiberId;
use extension_trait::extension_trait;

#[extension_trait]
pub impl FiberIdThreadIdConversion for FiberId {
    fn from_thread_id(id: usize) -> Self {
        Self::from_usize(id)
    }

    fn to_thread_id(self) -> usize {
        self.to_usize()
    }
}
