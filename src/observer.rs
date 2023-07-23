use crate::util::hash_sokoban_state;
use libafl::inputs::UsesInput;
use libafl::observers::{Observer, ObserverWithHashField};
use libafl::prelude::Named;
use libafl::Error;
use serde::{Deserialize, Serialize};
use sokoban::State as SokobanState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SokobanStateObserver {
    last_state: Option<SokobanState>,
    include_player: bool,
    name: String,
}

impl Named for SokobanStateObserver {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SokobanStateObserver {
    pub fn new(name: &str, include_player: bool) -> Self {
        Self {
            last_state: None,
            include_player,
            name: name.to_string(),
        }
    }

    pub fn replace(&mut self, state: SokobanState) -> Option<SokobanState> {
        self.last_state.replace(state)
    }

    pub fn last_state(&self) -> Option<&SokobanState> {
        self.last_state.as_ref()
    }
}

impl<S> Observer<S> for SokobanStateObserver
where
    S: UsesInput,
{
    fn flush(&mut self) -> Result<(), Error> {
        self.last_state = None;
        Ok(())
    }

    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        self.last_state = None;
        Ok(())
    }
}

impl ObserverWithHashField for SokobanStateObserver {
    fn hash(&self) -> Option<u64> {
        self.last_state
            .as_ref()
            .map(|state| hash_sokoban_state(state, self.include_player))
    }
}

pub trait SokobanObserversTuple {
    fn sokoban_observer_name(&self) -> &str;
}

impl<OT> SokobanObserversTuple for (SokobanStateObserver, OT) {
    fn sokoban_observer_name(&self) -> &str {
        self.0.name()
    }
}
