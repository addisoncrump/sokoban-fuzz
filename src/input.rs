use crate::state::InitialPuzzleMetadata;
use libafl::corpus::{CorpusId, Testcase};
use libafl::inputs::Input;
use libafl::prelude::HasCorpus;
use libafl::stages::mutational::MutatedTransform;
use libafl::state::HasMetadata;
use libafl::Error;
use serde::{Deserialize, Serialize};
use sokoban::{Direction, State as SokobanState};

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

    pub fn moves(&self) -> &[Direction] {
        &self.moves
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct HallucinatedSokobanInput {
    hallucinated: Option<SokobanState>,
    moves: Vec<Direction>,
}

impl HallucinatedSokobanInput {
    pub fn moves(&self) -> &Vec<Direction> {
        &self.moves
    }

    pub fn hallucinated_mut(&mut self) -> &mut Option<SokobanState> {
        &mut self.hallucinated
    }

    pub fn moves_mut(&mut self) -> &mut Vec<Direction> {
        &mut self.moves
    }
}

impl<S> MutatedTransform<SokobanInput, S> for HallucinatedSokobanInput
where
    S: HasCorpus<Input = SokobanInput> + HasMetadata,
{
    type Post = ();

    fn try_transform_from(
        base: &mut Testcase<SokobanInput>,
        state: &S,
        _corpus_idx: CorpusId,
    ) -> Result<Self, Error> {
        let hallucinated = state
            .metadata::<InitialPuzzleMetadata>()
            .unwrap()
            .initial()
            .clone();
        let input = base.load_input(state.corpus())?;
        let hallucinated = input
            .moves()
            .iter()
            .copied()
            .try_fold(hallucinated, |puzzle, direction| {
                puzzle.move_player(direction)
            })
            .expect("Invalid sequence of moves while performing transform!");

        Ok(Self {
            hallucinated: Some(hallucinated),
            moves: input.moves.clone(),
        })
    }

    fn try_transform_into(self, _state: &S) -> Result<(SokobanInput, Self::Post), Error> {
        Ok((SokobanInput::new(self.moves), ()))
    }
}
