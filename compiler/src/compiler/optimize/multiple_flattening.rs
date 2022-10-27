use crate::compiler::mir::Mir;

impl Mir {
    pub fn flatten_multiples(&mut self) {
        // For effiency reasons, flattening multiples operates directly on the
        // body's internal state and is thus defined directly in the MIR module.
        self.body.flatten_multiples();
    }
}
