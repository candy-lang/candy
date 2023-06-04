use candy_frontend::id::CountableId;
use candy_vm::fiber::FiberId;
use extension_trait::extension_trait;

#[extension_trait]
pub impl FiberIdThreadIdConversion for FiberId {
    fn from_thread_id(id: i64) -> Self {
        Self::from_usize(id as usize)
    }

    fn to_thread_id(self) -> i64 {
        self.to_usize() as i64
    }
}
