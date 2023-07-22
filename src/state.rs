use libafl::impl_serdeany;
use serde::{Deserialize, Serialize};
use sokoban::State as SokobanState;

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
