use crate::input::SokobanInput;
use crate::mutators::SokobanRemainingMutationsMetadata;
use crate::state::InitialPuzzleMetadata;
use crate::util::find_crates;
use libafl::corpus::{Corpus, CorpusId, HasTestcase};
use libafl::inputs::UsesInput;
use libafl::observers::ObserversTuple;
use libafl::prelude::Rand;
use libafl::schedulers::Scheduler;
use libafl::state::{HasCorpus, UsesState};
use libafl::state::{HasMetadata, HasRand};
use libafl::{impl_serdeany, Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SokobanWeightSchedulerMetadata {
    weight: HashMap<CorpusId, usize>,
    max_weight: usize,
    total_weight: usize,
    pruneable: Vec<CorpusId>,
}

impl_serdeany!(SokobanWeightSchedulerMetadata);

pub struct SokobanWeightScheduler<S> {
    phantom: PhantomData<S>,
}

impl<S> SokobanWeightScheduler<S>
where
    S: HasMetadata,
{
    pub fn new(state: &mut S) -> Self {
        state.add_metadata(SokobanWeightSchedulerMetadata::default());
        Self {
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
        let mut testcase = state.testcase_mut(idx)?;
        let input = testcase.load_input(state.corpus())?;
        let hallucinated = input
            .moves()
            .iter()
            .copied()
            .try_fold(
                state
                    .metadata::<InitialPuzzleMetadata>()
                    .unwrap()
                    .initial()
                    .clone(),
                |puzzle, direction| puzzle.move_player(direction),
            )
            .unwrap();

        let crates = find_crates(&hallucinated);

        let tc_meta = SokobanRemainingMutationsMetadata::new(&crates, hallucinated.targets());
        let remaining = tc_meta.remaining();

        testcase.add_metadata(tc_meta);
        drop(testcase);

        let metadata = state.metadata_mut::<SokobanWeightSchedulerMetadata>()?;
        metadata.weight.insert(idx, remaining);
        metadata.total_weight += remaining;
        if metadata.max_weight == 0 {
            metadata.max_weight = remaining;
        }

        Ok(())
    }

    fn on_evaluation<OT>(
        &mut self,
        state: &mut Self::State,
        _input: &<Self::State as UsesInput>::Input,
        _observers: &OT,
    ) -> Result<(), Error>
    where
        OT: ObserversTuple<Self::State>,
    {
        // we might be loading inputs
        if let &Some(current) = state.corpus().current() {
            let mut metadata = state
                .metadata_map_mut()
                .remove::<SokobanWeightSchedulerMetadata>()
                .unwrap();

            let testcase = state.testcase(current)?;
            let tc_meta = testcase.metadata::<SokobanRemainingMutationsMetadata>()?;
            let remaining = tc_meta.remaining();
            drop(testcase);

            let (computed, subtracted) = if remaining == 0 {
                let subtracted = metadata.weight.remove(&current).unwrap();
                metadata.pruneable.push(current);
                (0, subtracted)
            } else {
                // prefer deadending puzzles
                let computed = 1 + metadata.max_weight - remaining;
                let subtracted = metadata.weight.insert(current, computed).unwrap();
                (computed, subtracted)
            };

            metadata.total_weight += computed;
            metadata.total_weight -= subtracted;

            state.metadata_map_mut().insert_boxed(metadata);
        }
        Ok(())
    }

    fn next(&mut self, state: &mut Self::State) -> Result<CorpusId, Error> {
        let mut metadata = state
            .metadata_map_mut()
            .remove::<SokobanWeightSchedulerMetadata>()
            .unwrap();

        for pruneable in metadata.pruneable.drain(..) {
            state.corpus_mut().remove(pruneable)?;
        }

        let mut selected = state.rand_mut().below(metadata.total_weight as u64) as usize;
        for (&idx, &weight) in &metadata.weight {
            if let Some(next) = selected.checked_sub(weight) {
                selected = next;
            } else {
                state.metadata_map_mut().insert_boxed(metadata);
                state.corpus_mut().current_mut().replace(idx);
                return Ok(idx);
            }
        }
        unreachable!()
    }
}
