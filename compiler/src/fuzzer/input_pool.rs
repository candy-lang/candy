use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};

use super::{
    utils::Input,
    values::{generate_input, mutate_input},
};

pub type Score = f64;

pub struct InputPool {
    num_args: usize,
    inputs_and_scores: Vec<(Input, Score)>,
}

impl InputPool {
    pub fn new(num_args: usize) -> Self {
        Self {
            num_args,
            inputs_and_scores: vec![],
        }
    }

    pub fn generate_new_input(&self) -> Input {
        let mut rng = ThreadRng::default();

        if rng.gen_bool(0.1) || self.inputs_and_scores.len() < 20 {
            return generate_input(self.num_args);
        }

        let (input, _) = self
            .inputs_and_scores
            .choose_weighted(&mut rng, |(_, score)| *score)
            .unwrap();
        let mut input = input.clone();
        mutate_input(&mut rng, &mut input);
        return input;
    }

    pub fn add(&mut self, input: Input, score: Score) {
        self.inputs_and_scores.push((input, score));
    }
}
