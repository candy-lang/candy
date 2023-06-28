use super::input::Input;
use crate::{runner::RunResult, values::InputGeneration};
use candy_vm::heap::{Heap, Text};
use itertools::Itertools;
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cell::RefCell, rc::Rc};

pub type Score = f64;

pub struct InputPool {
    heap: Rc<RefCell<Heap>>,
    num_args: usize,
    symbols: Vec<Text>,
    results_and_scores: FxHashMap<Input, (RunResult, Score)>,
}

impl InputPool {
    pub fn new(num_args: usize, symbols_in_heap: &FxHashSet<Text>) -> Self {
        let mut heap = Heap::default();

        let mut symbols = symbols_in_heap
            .iter()
            .map(|symbol| symbol.clone_to_heap(&mut heap).try_into().unwrap())
            .collect_vec();
        symbols.push(Text::create(&mut heap, "True"));
        symbols.push(Text::create(&mut heap, "False"));

        Self {
            heap: Rc::new(RefCell::new(heap)),
            num_args,
            symbols,
            results_and_scores: FxHashMap::default(),
        }
    }

    pub fn generate_new_input(&self) -> Input {
        loop {
            let input = self.generate_input();
            if !self.results_and_scores.contains_key(&input) {
                return input;
            }
        }
    }
    pub fn generate_input(&self) -> Input {
        let mut rng = ThreadRng::default();

        if rng.gen_bool(0.1) || self.results_and_scores.len() < 20 {
            return Input::generate(self.heap.clone(), self.num_args, &self.symbols);
        }

        let inputs_and_scores = self.results_and_scores.iter().collect_vec();
        let (input, _) = inputs_and_scores
            .choose_weighted(&mut rng, |(_, (_, score))| *score)
            .unwrap();
        let mut input = (**input).clone();
        input.mutate(&mut rng, &self.symbols);
        input
    }

    pub fn add(&mut self, input: Input, result: RunResult, score: Score) {
        self.results_and_scores.insert(input, (result, score));
    }

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
}
