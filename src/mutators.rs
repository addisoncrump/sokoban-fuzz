use crate::input::SokobanInput;
use crate::state::InitialPuzzleMetadata;
use libafl::mutators::{MutationResult, Mutator};
use libafl::prelude::Rand;
use libafl::state::{HasMaxSize, HasMetadata, HasRand};
use libafl::Error;
use sokoban::Direction::*;
use sokoban::Tile::Crate;
use sokoban::{Direction, State as SokobanState, Tile};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

static POSSIBLE_MOVES: [Direction; 4] = [Up, Down, Left, Right];

pub struct AddMoveMutator;

impl<S> Mutator<SokobanInput, S> for AddMoveMutator
where
    S: HasMaxSize + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut SokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let dir = state.rand_mut().choose(POSSIBLE_MOVES);
        input.moves_mut().push(dir);

        Ok(MutationResult::Mutated)
    }
}

fn find_crates(puzzle: &SokobanState) -> Vec<(usize, usize)> {
    puzzle
        .iter()
        .filter(|item| item.tile() == Crate)
        .map(|item| item.position())
        .collect()
}

fn explore_local(
    start: (usize, usize),
    destination: (usize, usize),
    puzzle: &SokobanState,
    prev_moves: &mut HashMap<(usize, usize), Option<((usize, usize), Direction)>>,
    new_moves: &mut Vec<(usize, usize)>,
) -> bool {
    for direction in POSSIBLE_MOVES {
        if let Some(next) = direction.go(start) {
            if next.0 < puzzle.rows() && next.1 < puzzle.cols() && puzzle[next] == Tile::Floor {
                match prev_moves.entry(next) {
                    Entry::Occupied(_) => continue, // avoid backtracking
                    Entry::Vacant(e) => {
                        e.insert(Some((start, direction)));
                        if next == destination {
                            return true;
                        }
                        new_moves.push(next);
                    }
                }
            }
        }
    }
    false
}

// this implements a bit of a strange flood-fill with backreferences to get the previous
// moves taken
fn go_to(destination: (usize, usize), puzzle: &SokobanState) -> Option<VecDeque<Direction>> {
    if destination.0 < puzzle.rows()
        && destination.1 < puzzle.cols()
        && puzzle[destination] == Tile::Floor
    {
        if puzzle.player() == destination {
            return Some(VecDeque::new());
        }

        let mut prev_moves = HashMap::new();
        prev_moves.insert(puzzle.player(), None);
        let mut new_moves = Vec::new();

        // initialize the search
        explore_local(
            puzzle.player(),
            destination,
            puzzle,
            &mut prev_moves,
            &mut new_moves,
        );

        while !new_moves.is_empty() {
            let mut last_moves = Vec::new();
            core::mem::swap(&mut new_moves, &mut last_moves);
            for prev in last_moves {
                if explore_local(prev, destination, puzzle, &mut prev_moves, &mut new_moves) {
                    let mut moves = VecDeque::new();
                    let mut next = destination;
                    // walk backwards through the flood-fill
                    while let Some(&Some((prev, direction))) = prev_moves.get(&next) {
                        next = prev;
                        moves.push_front(direction);
                    }
                    return Some(moves);
                }
            }
        }
    }
    None
}

pub struct MoveCrateMutator;

const MAX_TRIES: usize = 16;

impl<S> Mutator<SokobanInput, S> for MoveCrateMutator
where
    S: HasMaxSize + HasMetadata + HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut SokobanInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if state.max_size() <= input.moves().len() {
            return Ok(MutationResult::Skipped);
        }

        let puzzle = state.metadata::<InitialPuzzleMetadata>()?.initial().clone();
        let current = input
            .moves()
            .iter()
            .cloned()
            .try_fold(puzzle, |puzzle, dir| puzzle.move_player(dir))
            .expect("Input provided was not valid.");

        // first, find the crates in the current puzzle state
        let crates = find_crates(&current);

        // try to move a random crate in a random direction
        for _ in 0..MAX_TRIES {
            let target = *state.rand_mut().choose(&crates);
            let direction = state.rand_mut().choose(POSSIBLE_MOVES);
            let opposite = match direction {
                Up => Down,
                Down => Up,
                Left => Right,
                Right => Left,
            };
            if let Some(destination) = opposite.go(target) {
                if let Some(moves) = go_to(destination, &current) {
                    input.moves_mut().extend(moves);
                    input.moves_mut().push(direction);
                    return Ok(MutationResult::Mutated);
                }
            }
        }

        Ok(MutationResult::Skipped)
    }
}

#[cfg(test)]
mod test {
    use crate::mutators::go_to;
    use sokoban::State as SokobanState;

    #[test]
    fn test_go_to_simple() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#__________________#
#__________________#
#__________________#
#_____________x____#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
#__________________#
####################
"#[..],
        )
        .unwrap();

        let moves = go_to((15, 3), &puzzle).expect("Couldn't find path to (15, 15)!");
        println!("{:?}", moves);
        let puzzle = moves
            .into_iter()
            .try_fold(puzzle, |puzzle, direction| puzzle.move_player(direction))
            .expect("Should not make invalid moves!");

        assert_eq!((15, 3), puzzle.player());
    }

    #[test]
    fn test_go_to_around_wall() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#________#_________#
#________#_________#
#________#____x____#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#__________________#
####################
"#[..],
        )
        .unwrap();

        let moves = go_to((3, 3), &puzzle).expect("Couldn't find path to (15, 15)!");
        println!("{:?}", moves);
        let puzzle = moves
            .into_iter()
            .try_fold(puzzle, |puzzle, direction| puzzle.move_player(direction))
            .expect("Should not make invalid moves!");

        assert_eq!((3, 3), puzzle.player());
    }

    #[test]
    fn test_go_to_impossible() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#________#_________#
#________#_________#
#________#____x____#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
####################
"#[..],
        )
        .unwrap();

        assert!(go_to((3, 3), &puzzle).is_none());
    }

    #[test]
    fn test_go_to_bad_dest() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#________#_________#
#________#_________#
#________#____x____#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
#________#_________#
####################
"#[..],
        )
        .unwrap();

        assert!(go_to((0, 0), &puzzle).is_none());
    }
}
