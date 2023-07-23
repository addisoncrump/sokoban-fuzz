use crate::input::SokobanInput;
use crate::state::InitialPuzzleMetadata;
use libafl::corpus::{CorpusId, HasTestcase};
use libafl::inputs::UsesInput;
use libafl::prelude::Rand;
use libafl::schedulers::Scheduler;
use libafl::state::{HasCorpus, UsesState};
use libafl::state::{HasMetadata, HasRand};
use libafl::Error;
use sokoban::Tile;
use std::collections::HashSet;
use std::marker::PhantomData;

pub struct SokobanWeightScheduler<S> {
    options: Vec<CorpusId>,
    available: Vec<CorpusId>,
    phantom: PhantomData<S>,
}

impl<S> SokobanWeightScheduler<S> {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            available: Vec::new(),
            phantom: PhantomData,
        }
    }
}

impl<S> UsesState for SokobanWeightScheduler<S>
where
    S: UsesInput,
{
    type State = S;
}

impl<S> Scheduler for SokobanWeightScheduler<S>
where
    S: HasCorpus<Input = SokobanInput> + HasMetadata + HasRand + HasTestcase,
{
    fn on_add(&mut self, state: &mut Self::State, idx: CorpusId) -> Result<(), Error> {
        let puzzle = state
            .metadata::<InitialPuzzleMetadata>()
            .unwrap()
            .initial()
            .clone();
        let mut testcase = state.testcase_mut(idx)?;
        let input = testcase.load_input(state.corpus())?;
        let hallucinated = input
            .moves()
            .iter()
            .copied()
            .try_fold(puzzle, |puzzle, direction| puzzle.move_player(direction))
            .unwrap();

        let weight = hallucinated
            .targets()
            .iter()
            .filter(|&&target| hallucinated[target] == Tile::Crate)
            .count()
            + 1;

        self.options.extend(std::iter::repeat(idx).take(weight));
        self.available.push(idx);

        Ok(())
    }

    fn next(&mut self, state: &mut Self::State) -> Result<CorpusId, Error> {
        if state.rand_mut().below(100) < 10 {
            Ok(*state.rand_mut().choose(&self.available))
        } else {
            Ok(*state.rand_mut().choose(&self.options))
        }
    }
}
