use crate::input::SokobanInput;
use crate::mutators::SokobanRemainingMutationsMetadata;
use crate::state::InitialPuzzleMetadata;
use crate::util::find_crates;
use libafl::corpus::{Corpus, CorpusId, HasTestcase};
use libafl::inputs::UsesInput;
use libafl::schedulers::Scheduler;
use libafl::state::{HasCorpus, UsesState};
use libafl::state::{HasMetadata, HasRand};
use libafl::Error;
use std::marker::PhantomData;

pub struct SokobanWeightScheduler<S> {
    phantom: PhantomData<S>,
}

impl<S> SokobanWeightScheduler<S>
where
    S: HasMetadata,
{
    pub fn new() -> Self {
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

        testcase.add_metadata(tc_meta);
        drop(testcase);

        Ok(())
    }

    fn next(&mut self, state: &mut Self::State) -> Result<CorpusId, Error> {
        if let &Some(current) = state.corpus().current() {
            let testcase = state.testcase(current)?;
            let tc_meta = testcase.metadata::<SokobanRemainingMutationsMetadata>()?;
            let remaining = tc_meta.remaining();
            drop(testcase);

            if remaining == 0 {
                state.corpus_mut().remove(current)?;
            } else {
                return Ok(current); // no change; keep fuzzing!
            }
        };

        let next = state.corpus().first().ok_or_else(|| {
            self.set_current_scheduled(state, None).unwrap();
            Error::key_not_found(format!(
                "Missing corpus entry; is the corpus empty? Reported size: {}",
                state.corpus().count()
            ))
        })?;
        self.set_current_scheduled(state, Some(next))?;
        Ok(next)
    }
}
