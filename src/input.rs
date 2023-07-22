use libafl::inputs::Input;
use serde::{Deserialize, Serialize};
use sokoban::Direction;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SokobanInput {
    moves: Vec<Direction>,
}

impl Input for SokobanInput {
    fn generate_name(&self, _idx: usize) -> String {
        String::from_utf8(
            self.moves
                .iter()
                .cloned()
                .map(|m| match m {
                    Direction::Up => b'U',
                    Direction::Down => b'D',
                    Direction::Left => b'L',
                    Direction::Right => b'R',
                })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
}

impl SokobanInput {
    pub fn new(moves: Vec<Direction>) -> Self {
        Self { moves }
    }

    pub fn moves_mut(&mut self) -> &mut Vec<Direction> {
        &mut self.moves
    }

    pub fn moves(&self) -> &[Direction] {
        &self.moves
    }
}
