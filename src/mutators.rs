use crate::input::SokobanInput;
use libafl::mutators::{MutationResult, Mutator};
use libafl::prelude::Rand;
use libafl::state::{HasMaxSize, HasRand};
use libafl::Error;
use sokoban::Direction::*;

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

        let dir = state.rand_mut().choose([Up, Down, Left, Right]);
        input.moves_mut().push(dir);

        Ok(MutationResult::Mutated)
    }
}
