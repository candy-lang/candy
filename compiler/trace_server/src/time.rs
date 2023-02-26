use std::time::Instant;

// Seconds since the start of the trace.
#[derive(Clone, Copy)]
pub struct Time(f64);

pub struct Timer {
    start: Instant,
}
impl Timer {
    pub fn start() -> Self {
        Timer {
            start: Instant::now(),
        }
    }
    pub fn get_time(&self) -> Time {
        let now = Instant::now();
        let elapsed = now - self.start;
        Time(elapsed.as_secs_f64())
    }
}
