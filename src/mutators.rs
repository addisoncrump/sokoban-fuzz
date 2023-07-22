use crate::input::HallucinatedSokobanInput;
use crate::util;
use crate::util::{opposite, push_to, POSSIBLE_MOVES};
use libafl::corpus::Corpus;
use libafl::mutators::{MutationResult, Mutator, MutatorsTuple};
use libafl::prelude::{CorpusId, MutationId, Named, Rand};
use libafl::state::{HasCorpus, HasMaxSize, HasMetadata, HasRand};
use libafl::Error;
use sokoban::error::SokobanError::{InvalidMoveCrate, InvalidMoveOOB, InvalidMoveWall};
use sokoban::Tile;
use std::collections::HashSet;

pub struct AddMoveMutator;

impl Named for AddMoveMutator {
    fn name(&self) -> &str {
        "move"
    }
}

impl<S> Mutator<HallucinatedSokobanInput, S> for AddMoveMutator
where
    S: HasMaxSize + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut HallucinatedSokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }
        let hallucinated = input.hallucinated_mut().take().unwrap();
        if hallucinated.in_solution_state() {
            input.hallucinated_mut().replace(hallucinated);
            return Ok(MutationResult::Skipped);
        }

        let dir = state.rand_mut().choose(POSSIBLE_MOVES);
        match hallucinated.move_player(dir) {
            Ok(hallucinated) => {
                input.hallucinated_mut().replace(hallucinated);
                input.moves_mut().push(dir);
                Ok(MutationResult::Mutated)
            }
            Err(
                InvalidMoveWall { last_state, .. }
                | InvalidMoveCrate { last_state, .. }
                | InvalidMoveOOB { last_state, .. },
            ) => {
                input.hallucinated_mut().replace(last_state);
                Ok(MutationResult::Skipped)
            }
            Err(_) => unreachable!(),
        }
    }
}

pub struct MoveCrateMutator;

impl Named for MoveCrateMutator {
    fn name(&self) -> &str {
        "move_crate"
    }
}

const MAX_TRIES: usize = 16;

impl<S> Mutator<HallucinatedSokobanInput, S> for MoveCrateMutator
where
    S: HasMaxSize + HasMetadata + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut HallucinatedSokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let current = input.hallucinated_mut().take().unwrap();
        if current.in_solution_state() {
            input.hallucinated_mut().replace(current);
            return Ok(MutationResult::Skipped);
        }

        // first, find the crates in the current puzzle state
        let crates = util::find_crates(&current);

        // try to move a random crate in a random direction
        for _ in 0..MAX_TRIES {
            let target = *state.rand_mut().choose(&crates);
            let direction = state.rand_mut().choose(POSSIBLE_MOVES);
            if let Some(destination) = opposite(direction).go(target) {
                if let Some(moves) = util::go_to(current.player(), destination, &current) {
                    input.hallucinated_mut().replace(
                        moves
                            .iter()
                            .copied()
                            .try_fold(current, |current, direction| current.move_player(direction))
                            .unwrap(),
                    );
                    input.moves_mut().extend(moves);
                    input.moves_mut().push(direction);
                    return Ok(MutationResult::Mutated);
                }
            }
        }

        input.hallucinated_mut().replace(current);

        Ok(MutationResult::Skipped)
    }
}

pub struct MoveCrateToTargetMutator;

impl Named for MoveCrateToTargetMutator {
    fn name(&self) -> &str {
        "move_crate_to_target"
    }
}

impl<S> Mutator<HallucinatedSokobanInput, S> for MoveCrateToTargetMutator
where
    S: HasMaxSize + HasMetadata + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut HallucinatedSokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let current = input.hallucinated_mut().take().unwrap();
        if current.in_solution_state() {
            input.hallucinated_mut().replace(current);
            return Ok(MutationResult::Skipped);
        }

        // first, find the crates in the current puzzle state
        let crates = util::find_crates(&current);

        // find the targets which are not occupied by crates
        let targets = current
            .targets()
            .iter()
            .cloned()
            .filter(|&target| current[target] == Tile::Floor)
            .collect::<Vec<_>>();

        // computing paths is really expensive, so remember which we tried already
        let mut attempted_pairs = HashSet::new();

        // try to move a random crate in a random direction
        for _ in 0..MAX_TRIES {
            let moved = *state.rand_mut().choose(&crates);
            let target = *state.rand_mut().choose(&targets);

            if !attempted_pairs.insert((moved, target)) {
                if let Some(moves) = push_to(moved, target, &current) {
                    input.hallucinated_mut().replace(
                        moves
                            .iter()
                            .copied()
                            .try_fold(current, |current, direction| current.move_player(direction))
                            .unwrap(),
                    );
                    input.moves_mut().extend(moves);
                    return Ok(MutationResult::Mutated);
                }
            }
        }

        input.hallucinated_mut().replace(current);

        Ok(MutationResult::Skipped)
    }
}

const WEIGHT_PRECISION: u64 = 128;

pub struct RandomPreferenceMutator<MT> {
    mutators: MT,
    weights: Vec<MutationId>,
    total_weight: usize,
    until_reweight: usize,
    last_id: CorpusId,
}

impl<MT> Named for RandomPreferenceMutator<MT> {
    fn name(&self) -> &str {
        "random_preference"
    }
}

impl<MT> RandomPreferenceMutator<MT> {
    pub fn new(mutators: MT) -> Self {
        Self {
            mutators,
            weights: Vec::new(),
            total_weight: 0,
            until_reweight: 0,
            last_id: CorpusId::from(0u64),
        }
    }
}

impl<I, MT, S> Mutator<I, S> for RandomPreferenceMutator<MT>
where
    MT: MutatorsTuple<I, S>,
    S: HasCorpus + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if self.until_reweight == 0 {
            self.until_reweight = state.corpus().count();
            self.total_weight = 0;
            self.weights.clear();
            for i in 0..self.mutators.len() {
                let amount = 1 + state.rand_mut().below(WEIGHT_PRECISION) as usize;
                self.weights
                    .extend(std::iter::repeat(MutationId::from(i)).take(amount));
                self.total_weight += amount;
            }
        } else if state
            .corpus()
            .current()
            .map_or(false, |id| id != self.last_id)
        {
            self.until_reweight -= 1;
        }

        let idx = state.rand_mut().below(self.total_weight as u64) as usize;
        let idx = self.weights[idx];

        self.mutators.get_and_mutate(idx, state, input, stage_idx)
    }
}
