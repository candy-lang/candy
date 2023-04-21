use super::{
    input::Input,
    values::{generate_input, generate_mutated_input},
};
use candy_vm::heap::{Heap, Symbol};
use itertools::Itertools;
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cell::RefCell, rc::Rc};

pub type Score = f64;

pub struct InputPool {
    heap: Rc<RefCell<Heap>>,
    num_args: usize,
    symbols: Vec<Symbol>,
    input_scores: FxHashMap<Input, Score>,
}

impl InputPool {
    pub fn new(num_args: usize, symbols_in_heap: &FxHashSet<String>) -> Self {
        let mut heap = Heap::default();

        let mut symbols = symbols_in_heap
            .iter()
            .map(|symbol| Symbol::create(&mut heap, symbol))
            .collect_vec();
        if symbols.is_empty() {
            symbols.push(Symbol::create_nothing(&mut heap));
        }

        Self {
            heap: Rc::new(RefCell::new(heap)),
            num_args,
            symbols,
            input_scores: FxHashMap::default(),
        }
    }

    pub fn generate_new_input(&self) -> Input {
        loop {
            let input = self.generate_input();
            if !self.input_scores.contains_key(&input) {
                return input;
            }
        }
    }
    pub fn generate_input(&self) -> Input {
        let mut rng = ThreadRng::default();

        if rng.gen_bool(0.1) || self.input_scores.len() < 20 {
            return generate_input(self.heap.clone(), self.num_args, &self.symbols);
        }

        let inputs_and_scores = self.input_scores.iter().collect_vec();
        let (input, _) = inputs_and_scores
            .choose_weighted(&mut rng, |(_, score)| *score)
            .unwrap();
        let mut input = (**input).clone();
        generate_mutated_input(&mut rng, &mut input, &self.symbols);
        input
    }

    pub fn add(&mut self, input: Input, score: Score) {
        self.input_scores.insert(input, score);
    }
}
