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
    more_set: Vec<CorpusId>,
    inverse: Vec<CorpusId>,
    length: Vec<CorpusId>,
    equal: Vec<CorpusId>,
}

impl_serdeany!(SokobanWeightSchedulerMetadata);

pub struct SokobanWeightScheduler<S> {
    obs_name: String,
    last_targets: Option<usize>,
    last_len: Option<usize>,
    phantom: PhantomData<S>,
}

impl<S> SokobanWeightScheduler<S> {
    pub fn new(obs: &SokobanStateObserver) -> Self {
        Self {
            obs_name: obs.name().to_string(),
            last_targets: None,
            last_len: None,
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
            .more_set
            .extend(std::iter::repeat(idx).take(weight + 1));
        metadata
            .inverse
            .extend(std::iter::repeat(idx).take(inverse));
        metadata
            .length
            .extend(std::iter::repeat(idx).take(1 + self.last_len.unwrap()));
        metadata.equal.push(idx);

        Ok(())
    }

    fn on_evaluation<OT>(
        &mut self,
        _state: &mut Self::State,
        input: &<Self::State as UsesInput>::Input,
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
        self.last_len = Some(input.moves().len());
        Ok(())
    }

    fn next(&mut self, state: &mut Self::State) -> Result<CorpusId, Error> {
        let metadata = state
            .metadata_map_mut()
            .remove::<SokobanWeightSchedulerMetadata>()
            .unwrap();
        let coin = state.rand_mut().next();
        let selected = if coin < u64::MAX / 4 {
            *state.rand_mut().choose(&metadata.inverse)
        } else if coin < u64::MAX / 2 {
            *state.rand_mut().choose(&metadata.more_set)
        } else if coin / 3 < u64::MAX / 4 {
            *state.rand_mut().choose(&metadata.length)
        } else {
            *state.rand_mut().choose(&metadata.equal)
        };
        state.metadata_map_mut().insert_boxed(metadata);
        Ok(selected)
    }
}
