use super::input::Input;
use crate::runner::RunResult;
use candy_vm::heap::{Heap, Text};
use itertools::Itertools;
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};
use rustc_hash::FxHashMap;

pub type Score = f64;

pub struct InputPool {
    num_args: usize,
    symbols: Vec<Text>,
    results_and_scores: FxHashMap<Input, (RunResult, Score)>,
}

impl InputPool {
    #[must_use]
    pub fn new(num_args: usize, symbols: Vec<Text>) -> Self {
        Self {
            num_args,
            symbols,
            results_and_scores: FxHashMap::default(),
        }
    }

    #[must_use]
    pub fn generate_new_input(&self, heap: &mut Heap) -> Input {
        loop {
            let input = self.generate_input(heap);
            if self.results_and_scores.contains_key(&input) {
                input.drop(heap);
                continue;
            }

            return input;
        }
    }
    #[must_use]
    pub fn generate_input(&self, heap: &mut Heap) -> Input {
        let mut rng = ThreadRng::default();

        if rng.gen_bool(0.1) || self.results_and_scores.len() < 20 {
            return Input::generate(heap, self.num_args, &self.symbols);
        }

        let inputs_and_scores = self
            .results_and_scores
            .iter()
            .map(|(input, (_, score))| (input, *score))
            .collect_vec();
        let (input, _) = inputs_and_scores
            .choose_weighted(&mut rng, |(_, score)| *score)
            .unwrap();
        input.mutated(heap, &mut rng, &self.symbols)
    }

    pub fn add(&mut self, input: Input, result: RunResult, score: Score) {
        self.results_and_scores.insert(input, (result, score));
    }

    #[must_use]
    pub fn interesting_inputs(&self) -> Vec<Input> {
        self.results_and_scores
            .iter()
            .sorted_by(
                |(_, (result_a, mut score_a)), (_, (result_b, mut score_b))| {
                    if matches!(result_a, RunResult::Done { .. }) {
                        score_a += 50.;
                    }
                    if matches!(result_b, RunResult::Done { .. }) {
                        score_b += 50.;
                    }
                    score_a.partial_cmp(&score_b).unwrap()
                },
            )
            .rev()
            .take(3)
            .map(|(input, _)| input.clone())
            .collect_vec()
    }

    pub fn result_of(&self, input: &Input) -> &RunResult {
        &self.results_and_scores.get(input).unwrap().0
    }

    pub fn drop(self, heap: &mut Heap) {
        for symbol in self.symbols {
            symbol.drop(heap);
        }
        for (input, _) in self.results_and_scores {
            input.drop(heap);
        }
    }
}
