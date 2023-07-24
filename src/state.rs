use libafl::impl_serdeany;
use serde::{Deserialize, Serialize};
use sokoban::State as SokobanState;
use std::cell::{RefCell, RefMut};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitialPuzzleMetadata {
    initial: SokobanState,
}

impl_serdeany!(InitialPuzzleMetadata);

impl InitialPuzzleMetadata {
    pub fn new(initial: SokobanState) -> Self {
        Self { initial }
    }

    pub fn initial(&self) -> &SokobanState {
        &self.initial
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LastHallucinationMetadata {
    hallucination: RefCell<Option<SokobanState>>,
}

impl_serdeany!(LastHallucinationMetadata);

impl LastHallucinationMetadata {
    pub fn hallucination_mut(&self) -> RefMut<Option<SokobanState>> {
        self.hallucination.borrow_mut()
    }
}
