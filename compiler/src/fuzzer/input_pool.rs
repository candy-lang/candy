use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};

use super::{
    input::Input,
    values::{generate_input, mutate_input},
};

pub type Score = f64;

pub struct InputPool {
    num_args: usize,
    symbols: Vec<String>,
    inputs_and_scores: Vec<(Input, Score)>,
}

impl InputPool {
    pub fn new(num_args: usize, symbols_in_heap: Vec<String>) -> Self {
        Self {
            num_args,
            symbols: symbols_in_heap,
            inputs_and_scores: vec![],
        }
    }

    pub fn generate_new_input(&self) -> Input {
        let mut rng = ThreadRng::default();

        if rng.gen_bool(0.1) || self.inputs_and_scores.len() < 20 {
            return generate_input(self.num_args, &self.symbols);
        }

        let (input, _) = self
            .inputs_and_scores
            .choose_weighted(&mut rng, |(_, score)| *score)
            .unwrap();
        let mut input = input.clone();
        mutate_input(&mut rng, &mut input, &self.symbols);
        input
    }

    pub fn add(&mut self, input: Input, score: Score) {
        self.inputs_and_scores.push((input, score));
    }
}
