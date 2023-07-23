use crate::input::HallucinatedSokobanInput;
use crate::util;
use crate::util::{opposite, push_to, POSSIBLE_MOVES};
use libafl::corpus::{Corpus, HasTestcase};
use libafl::mutators::{MutationResult, Mutator, MutatorsTuple};
use libafl::prelude::{MutationId, Named, Rand};
use libafl::state::{HasCorpus, HasMaxSize, HasMetadata, HasRand};
use libafl::{impl_serdeany, Error};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sokoban::error::SokobanError::{InvalidMoveCrate, InvalidMoveOOB, InvalidMoveWall};
use sokoban::Direction;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SokobanRemainingMutationsMetadata {
    moves_remaining: HashSet<((usize, usize), Direction)>,
    move_to_targets_remaining: HashSet<((usize, usize), (usize, usize))>,
}

impl_serdeany!(SokobanRemainingMutationsMetadata);

impl SokobanRemainingMutationsMetadata {
    pub fn new(crates: &[(usize, usize)], targets: &[(usize, usize)]) -> Self {
        let mut moves_remaining = HashSet::with_capacity(crates.len() * 4);
        let mut move_to_targets_remaining = HashSet::with_capacity(crates.len() * targets.len());
        for &moved in crates {
            for direction in POSSIBLE_MOVES {
                moves_remaining.insert((moved, direction));
            }
            for &target in targets {
                move_to_targets_remaining.insert((moved, target));
            }
        }
        Self {
            moves_remaining,
            move_to_targets_remaining,
        }
    }

    pub fn remaining(&self) -> usize {
        self.moves_remaining.len() + self.move_to_targets_remaining.len()
    }
}

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

impl<S> Mutator<HallucinatedSokobanInput, S> for MoveCrateMutator
where
    S: HasCorpus + HasMaxSize + HasMetadata + HasRand + HasTestcase,
    S::Rand: RngCore,
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

        // get the available mutations
        let selector = state.rand_mut().next() as usize;
        let idx = state.corpus().current().unwrap();
        let mut testcase = state.testcase_mut(idx)?;
        let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;

        if remaining.moves_remaining.is_empty() {
            return Ok(MutationResult::Skipped);
        }
        let selector = selector % remaining.moves_remaining.len();
        let entry = *remaining.moves_remaining.iter().nth(selector).unwrap();
        remaining.moves_remaining.remove(&entry);

        let (target, direction) = entry;

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
    S: HasCorpus + HasMaxSize + HasMetadata + HasRand + HasTestcase,
    S::Rand: RngCore,
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

        // get the available mutations
        let selector = state.rand_mut().next() as usize;
        let idx = state.corpus().current().unwrap();
        let mut testcase = state.testcase_mut(idx)?;
        let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;

        if remaining.move_to_targets_remaining.is_empty() {
            return Ok(MutationResult::Skipped);
        }
        let selector = selector % remaining.move_to_targets_remaining.len();
        let entry = *remaining
            .move_to_targets_remaining
            .iter()
            .nth(selector)
            .unwrap();
        remaining.move_to_targets_remaining.remove(&entry);

        let (moved, target) = entry;

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

        input.hallucinated_mut().replace(current);

        Ok(MutationResult::Skipped)
    }
}

const WEIGHT_PRECISION: u64 = 64;
const REWEIGHT_FREQUENCY: usize = 10_000;

pub struct RandomPreferenceMutator<MT> {
    mutators: MT,
    weights: Vec<MutationId>,
    total_weight: usize,
    until_reweight: usize,
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
            self.until_reweight = REWEIGHT_FREQUENCY;
            self.total_weight = 0;
            self.weights.clear();
            for i in 0..self.mutators.len() {
                let amount = 1 + state.rand_mut().below(WEIGHT_PRECISION) as usize;
                self.weights
                    .extend(std::iter::repeat(MutationId::from(i)).take(amount));
                self.total_weight += amount;
            }
        } else {
            self.until_reweight -= 1;
        }

        let idx = state.rand_mut().below(self.total_weight as u64) as usize;
        let idx = self.weights[idx];

        self.mutators.get_and_mutate(idx, state, input, stage_idx)
    }
}
