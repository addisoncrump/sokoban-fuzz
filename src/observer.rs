use libafl::inputs::UsesInput;
use libafl::observers::{Observer, ObserverWithHashField};
use libafl::prelude::Named;
use libafl::Error;
use serde::{Deserialize, Serialize};
use sokoban::{State as SokobanState, Tile};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Serialize, Deserialize)]
pub struct SokobanStateObserver {
    last_state: Option<SokobanState>,
    name: String,
}

impl Named for SokobanStateObserver {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SokobanStateObserver {
    pub fn new(name: &str) -> Self {
        Self {
            last_state: None,
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
        self.last_state.as_ref().map(|state| {
            let mut hasher = DefaultHasher::new();
            for item in state.iter().filter(|item| item.tile() == Tile::Crate) {
                item.position().hash(&mut hasher);
            }
            state.player().hash(&mut hasher);
            hasher.finish()
        })
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
