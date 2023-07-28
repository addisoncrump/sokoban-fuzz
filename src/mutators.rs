use crate::input::HallucinatedSokobanInput;
use crate::util;
use crate::util::{find_crates, opposite, push_to, POSSIBLE_MOVES};
use libafl::corpus::{Corpus, HasTestcase};
use libafl::mutators::{MutationResult, Mutator, MutatorsTuple};
use libafl::prelude::{MutationId, Named, Rand};
use libafl::state::{HasCorpus, HasMaxSize, HasMetadata, HasRand};
use libafl::{impl_serdeany, Error};
use rand::seq::SliceRandom;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sokoban::{Direction, Tile};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SokobanRemainingMutationsMetadata {
    moves_remaining: Vec<((usize, usize), Direction)>,
    move_to_targets_remaining: Vec<((usize, usize), (usize, usize))>,
}

impl_serdeany!(SokobanRemainingMutationsMetadata);

impl SokobanRemainingMutationsMetadata {
    pub fn new(crates: &[(usize, usize)], targets: &[(usize, usize)]) -> Self {
        let mut moves_remaining = Vec::with_capacity(crates.len() * 4);
        let mut move_to_targets_remaining = Vec::with_capacity(crates.len() * targets.len());
        for &moved in crates {
            for direction in POSSIBLE_MOVES {
                moves_remaining.push((moved, direction));
            }
            for &target in targets {
                move_to_targets_remaining.push((moved, target));
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
        let idx = state.corpus().current().unwrap();

        if state.max_size() <= input.moves().len() {
            let mut testcase = state.testcase_mut(idx)?;
            let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;
            remaining.moves_remaining.clear();
            return Ok(MutationResult::Skipped);
        }

        let current = input.hallucinated_mut().take().unwrap();

        loop {
            // get the available mutations
            let mut testcase = state.testcase_mut(idx)?;
            let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;

            if remaining.moves_remaining.is_empty() {
                input.hallucinated_mut().replace(current);
                return Ok(MutationResult::Skipped);
            }
            let (target, direction) = remaining.moves_remaining.pop().unwrap();

            if let Some(potential) = direction.go(target) {
                if current[potential] == Tile::Floor {
                    if let Some(destination) = opposite(direction).go(target) {
                        if let Some(moves) = util::go_to(current.player(), destination, &current) {
                            if moves.len() + input.moves().len() > state.max_size() {
                                input.hallucinated_mut().replace(current);
                                return Ok(MutationResult::Skipped);
                            }

                            input.hallucinated_mut().replace(
                                moves
                                    .iter()
                                    .copied()
                                    .try_fold(current, |current, direction| {
                                        current.move_player(direction)
                                    })
                                    .and_then(|current| current.move_player(direction))
                                    .unwrap(),
                            );
                            input.moves_mut().extend(moves);
                            input.moves_mut().push(direction);
                            return Ok(MutationResult::Mutated);
                        }
                    }
                }
            }
        }
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
        let idx = state.corpus().current().unwrap();

        if state.max_size() <= input.moves().len() {
            let mut testcase = state.testcase_mut(idx)?;
            let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;
            remaining.move_to_targets_remaining.clear();
            return Ok(MutationResult::Skipped);
        }

        let current = input.hallucinated_mut().take().unwrap();

        loop {
            // get the available mutations
            let mut testcase = state.testcase_mut(idx)?;
            let remaining = testcase.metadata_mut::<SokobanRemainingMutationsMetadata>()?;

            if remaining.move_to_targets_remaining.is_empty() {
                input.hallucinated_mut().replace(current);
                return Ok(MutationResult::Skipped);
            }
            let (moved, target) = remaining.move_to_targets_remaining.pop().unwrap();

            if let Some(moves) = push_to(moved, target, &current) {
                if moves.len() + input.moves().len() > state.max_size() {
                    input.hallucinated_mut().replace(current);
                    return Ok(MutationResult::Skipped);
                }

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
}

pub struct OneShotMutator;

impl Named for OneShotMutator {
    fn name(&self) -> &str {
        "move_many_crates_to_targets"
    }
}

impl<S> Mutator<HallucinatedSokobanInput, S> for OneShotMutator
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

        let mut current = input.hallucinated_mut().take().unwrap();
        if current.in_solution_state() {
            input.hallucinated_mut().replace(current);
            return Ok(MutationResult::Skipped);
        }

        let mut targets = current.targets().to_vec();
        targets.shuffle(state.rand_mut());

        let mut crates = find_crates(&current);
        crates.shuffle(state.rand_mut());

        let mut mutated = MutationResult::Skipped;

        for (target, moved) in targets.into_iter().zip(crates) {
            if let Some(moves) = push_to(moved, target, &current) {
                if moves.len() + input.moves().len() > state.max_size() {
                    break; // we may have already mutated the input
                }

                current = moves
                    .iter()
                    .copied()
                    .try_fold(current, |puzzle, direction| puzzle.move_player(direction))
                    .unwrap();
                input.moves_mut().extend(moves);
                mutated = MutationResult::Mutated;
            } else {
                break;
            }

            if current.in_solution_state() {
                break;
            }
        }

        input.hallucinated_mut().replace(current);

        Ok(mutated)
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
