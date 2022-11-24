#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct TracingConfig {
    pub register_fuzzables: bool,
    pub trace_calls: bool,
    pub trace_evaluated_expressions: bool,
}

impl TracingConfig {
    pub fn none() -> Self {
        Self {
            register_fuzzables: false,
            trace_calls: false,
            trace_evaluated_expressions: false,
        }
    }
}

pub trait AdjustCasingOfFirstLetter {
    fn lowercase_first_letter(&self) -> String;
    fn uppercase_first_letter(&self) -> String;
}
impl AdjustCasingOfFirstLetter for str {
    fn lowercase_first_letter(&self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        }
    }

    fn uppercase_first_letter(&self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }
}
