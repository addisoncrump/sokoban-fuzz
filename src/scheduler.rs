use crate::input::SokobanInput;
use crate::observer::SokobanStateObserver;
use crate::state::InitialPuzzleMetadata;
use libafl::corpus::CorpusId;
use libafl::inputs::UsesInput;
use libafl::observers::ObserversTuple;
use libafl::prelude::{Named, Rand};
use libafl::schedulers::Scheduler;
use libafl::state::{HasCorpus, UsesState};
use libafl::state::{HasMetadata, HasRand};
use libafl::{impl_serdeany, Error};
use serde::{Deserialize, Serialize};
use sokoban::Tile;
use std::marker::PhantomData;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SokobanWeightSchedulerMetadata {
    options: Vec<CorpusId>,
    inverse: Vec<CorpusId>,
}

impl_serdeany!(SokobanWeightSchedulerMetadata);

pub struct SokobanWeightScheduler<S> {
    obs_name: String,
    last_targets: Option<usize>,
    phantom: PhantomData<S>,
}

impl<S> SokobanWeightScheduler<S> {
    pub fn new(obs: &SokobanStateObserver) -> Self {
        Self {
            obs_name: obs.name().to_string(),
            last_targets: None,
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
    S: HasCorpus<Input = SokobanInput> + HasMetadata + HasRand,
{
    fn on_add(&mut self, state: &mut Self::State, idx: CorpusId) -> Result<(), Error> {
        let weight = self.last_targets.take().unwrap();
        let inverse = state
            .metadata::<InitialPuzzleMetadata>()
            .unwrap()
            .initial()
            .targets()
            .len()
            - weight;

        let metadata = if let Ok(value) = state.metadata_mut::<SokobanWeightSchedulerMetadata>() {
            value
        } else {
            state.add_metadata(SokobanWeightSchedulerMetadata::default());
            state
                .metadata_mut::<SokobanWeightSchedulerMetadata>()
                .unwrap()
        };

        metadata
            .options
            .extend(std::iter::repeat(idx).take(weight + 1));
        metadata
            .inverse
            .extend(std::iter::repeat(idx).take(inverse));

        Ok(())
    }

    fn on_evaluation<OT>(
        &mut self,
        _state: &mut Self::State,
        _input: &<Self::State as UsesInput>::Input,
        observers: &OT,
    ) -> Result<(), Error>
    where
        OT: ObserversTuple<Self::State>,
    {
        if let Some(last_state) = observers
            .match_name::<SokobanStateObserver>(&self.obs_name)
            .unwrap()
            .last_state()
        {
            self.last_targets = Some(
                last_state
                    .targets()
                    .iter()
                    .filter(|&&target| last_state[target] == Tile::Crate)
                    .count(),
            );
        }
        Ok(())
    }

    fn next(&mut self, state: &mut Self::State) -> Result<CorpusId, Error> {
        let metadata = state
            .metadata_map_mut()
            .remove::<SokobanWeightSchedulerMetadata>()
            .unwrap();
        let selected = if state.rand_mut().below(100) < 50 {
            *state.rand_mut().choose(&metadata.inverse)
        } else {
            *state.rand_mut().choose(&metadata.options)
        };
        state.metadata_map_mut().insert_boxed(metadata);
        Ok(selected)
    }
}
