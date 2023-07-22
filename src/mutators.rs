use crate::input::SokobanInput;
use crate::state::InitialPuzzleMetadata;
use crate::util;
use crate::util::{opposite, push_to, POSSIBLE_MOVES};
use libafl::mutators::{MutationResult, Mutator};
use libafl::prelude::{Named, Rand};
use libafl::state::{HasMaxSize, HasMetadata, HasRand};
use libafl::Error;
use sokoban::Tile;
use std::collections::{BTreeSet, HashSet};

pub struct AddMoveMutator;

impl<S> Mutator<SokobanInput, S> for AddMoveMutator
where
    S: HasMaxSize + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut SokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let dir = state.rand_mut().choose(POSSIBLE_MOVES);
        input.moves_mut().push(dir);

        Ok(MutationResult::Mutated)
    }
}

pub struct MoveCrateMutator;

impl Named for MoveCrateMutator {
    fn name(&self) -> &str {
        "move"
    }
}

const MAX_TRIES: usize = 16;

impl<S> Mutator<SokobanInput, S> for MoveCrateMutator
where
    S: HasMaxSize + HasMetadata + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut SokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let puzzle = state.metadata::<InitialPuzzleMetadata>()?.initial().clone();
        let current = input
            .moves()
            .iter()
            .cloned()
            .try_fold(puzzle, |puzzle, dir| puzzle.move_player(dir))
            .expect("Input provided was not valid.");

        // first, find the crates in the current puzzle state
        let crates = util::find_crates(&current);

        // try to move a random crate in a random direction
        for _ in 0..MAX_TRIES {
            let target = *state.rand_mut().choose(&crates);
            let direction = state.rand_mut().choose(POSSIBLE_MOVES);
            if let Some(destination) = opposite(direction).go(target) {
                if let Some(moves) = util::go_to(current.player(), destination, &current) {
                    input.moves_mut().extend(moves);
                    input.moves_mut().push(direction);
                    return Ok(MutationResult::Mutated);
                }
            }
        }

        Ok(MutationResult::Skipped)
    }
}

pub struct MoveCrateToTargetMutator;

impl Named for MoveCrateToTargetMutator {
    fn name(&self) -> &str {
        "move_to_target"
    }
}

impl<S> Mutator<SokobanInput, S> for MoveCrateToTargetMutator
where
    S: HasMaxSize + HasMetadata + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut SokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let puzzle = state.metadata::<InitialPuzzleMetadata>()?.initial().clone();
        let current = input
            .moves()
            .iter()
            .cloned()
            .try_fold(puzzle, |puzzle, dir| puzzle.move_player(dir))
            .expect("Input provided was not valid.");

        // first, find the crates in the current puzzle state
        let mut crates = util::find_crates(&current);

        // find the targets which are not occupied by crates
        let targets = current
            .targets()
            .iter()
            .cloned()
            .filter(|&target| current[target] == Tile::Floor)
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return Ok(MutationResult::Skipped); // we've solved the puzzle!
        }

        // computing paths is really expensive, so remember which we tried already
        let mut attempted_pairs = HashSet::new();

        // try to move a random crate in a random direction
        for _ in 0..MAX_TRIES {
            let moved = *state.rand_mut().choose(&crates);
            let target = *state.rand_mut().choose(&targets);

            if !attempted_pairs.insert((moved, target)) {
                if let Some(moves) = push_to(moved, target, &current) {
                    input.moves_mut().extend(moves);
                    return Ok(MutationResult::Mutated);
                }
            }
        }

        Ok(MutationResult::Skipped)
    }
}
