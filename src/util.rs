use sokoban::Direction::{Down, Left, Right, Up};
use sokoban::{Direction, State as SokobanState, Tile};
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};

pub static POSSIBLE_MOVES: [Direction; 4] = [Up, Down, Left, Right];

pub const fn opposite(dir: Direction) -> Direction {
    match dir {
        Up => Down,
        Down => Up,
        Left => Right,
        Right => Left,
    }
}

pub fn find_crates(puzzle: &SokobanState) -> Vec<(usize, usize)> {
    puzzle
        .iter()
        .filter(|item| item.tile() == Tile::Crate)
        .map(|item| item.position())
        .collect()
}

fn explore_local(
    start: (usize, usize),
    destination: (usize, usize),
    puzzle: &SokobanState,
    prev_moves: &mut HashMap<(usize, usize), Option<Direction>>,
    new_moves: &mut Vec<(usize, usize)>,
) -> bool {
    for direction in POSSIBLE_MOVES {
        if let Some(next) = direction.go(start) {
            if next.0 < puzzle.rows() && next.1 < puzzle.cols() && puzzle[next] == Tile::Floor {
                match prev_moves.entry(next) {
                    Entry::Occupied(_) => continue, // avoid backtracking
                    Entry::Vacant(e) => {
                        e.insert(Some(direction));
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
pub fn go_to(
    start: (usize, usize),
    destination: (usize, usize),
    puzzle: &SokobanState,
) -> Option<VecDeque<Direction>> {
    if start.0 < puzzle.rows()
        && start.1 < puzzle.cols()
        && puzzle[start] == Tile::Floor
        && destination.0 < puzzle.rows()
        && destination.1 < puzzle.cols()
        && puzzle[destination] == Tile::Floor
    {
        if start == destination {
            return Some(VecDeque::new());
        }

        let mut prev_moves = HashMap::new();
        prev_moves.insert(start, None);
        let mut new_moves = vec![start];

        while !new_moves.is_empty() {
            let mut last_moves = Vec::new();
            core::mem::swap(&mut new_moves, &mut last_moves);
            for prev in last_moves {
                if explore_local(prev, destination, puzzle, &mut prev_moves, &mut new_moves) {
                    let mut moves = VecDeque::new();
                    let mut next = destination;
                    // walk backwards through the flood-fill
                    while let Some(&Some(direction)) = prev_moves.get(&next) {
                        next = opposite(direction).go(next).unwrap();
                        moves.push_front(direction);
                    }
                    return Some(moves);
                }
            }
        }
    }
    None
}

// same as go_to but doesn't recover the path
pub fn can_go_to(
    start: (usize, usize),
    destination: (usize, usize),
    puzzle: &SokobanState,
) -> bool {
    if start.0 < puzzle.rows()
        && start.1 < puzzle.cols()
        && puzzle[start] == Tile::Floor
        && destination.0 < puzzle.rows()
        && destination.1 < puzzle.cols()
        && puzzle[destination] == Tile::Floor
    {
        if start == destination {
            return true;
        }

        let mut prev_moves = HashMap::new();
        prev_moves.insert(start, None);
        let mut new_moves = vec![start];

        while !new_moves.is_empty() {
            let mut last_moves = Vec::new();
            core::mem::swap(&mut new_moves, &mut last_moves);
            for prev in last_moves {
                if explore_local(prev, destination, puzzle, &mut prev_moves, &mut new_moves) {
                    return true;
                }
            }
        }
    }
    false
}

// same as explore_local, but makes sure that the player can get to the specified position
fn push_local(
    player: (usize, usize),
    start: (usize, usize),
    destination: (usize, usize),
    hallucinated: &mut SokobanState,
    prev_moves: &mut HashMap<(usize, usize), Option<Direction>>,
    new_moves: &mut Vec<(usize, usize)>,
) -> bool {
    for direction in POSSIBLE_MOVES {
        if let Some(next) = direction.go(start) {
            if let Some(push_point) = opposite(direction).go(start) {
                if next.0 < hallucinated.rows()
                    && next.1 < hallucinated.cols()
                    && hallucinated[next] == Tile::Floor
                    && push_point.0 < hallucinated.rows()
                    && push_point.1 < hallucinated.cols()
                    && hallucinated[push_point] == Tile::Floor
                {
                    match prev_moves.entry(next) {
                        Entry::Occupied(_) => continue, // avoid backtracking
                        Entry::Vacant(e) => {
                            // we need to hallucinate that the crate *is* there
                            hallucinated[start] = Tile::Crate;

                            // check that the player can get there
                            if can_go_to(player, push_point, hallucinated) {
                                e.insert(Some(direction));
                                if next == destination {
                                    hallucinated[start] = Tile::Floor;
                                    return true;
                                }
                                new_moves.push(next);
                            }
                            hallucinated[start] = Tile::Floor;
                        }
                    }
                }
            }
        }
    }
    false
}

// this is the same concept as go_to, but ensures the player can push at any point
pub fn push_to(
    start: (usize, usize),
    destination: (usize, usize),
    puzzle: &SokobanState,
) -> Option<Vec<Direction>> {
    if start.0 < puzzle.rows()
        && start.1 < puzzle.cols()
        && puzzle[start] == Tile::Crate
        && destination.0 < puzzle.rows()
        && destination.1 < puzzle.cols()
        && puzzle[destination] == Tile::Floor
    {
        if start == destination {
            return Some(Vec::new());
        }

        // we need to hallucinate that the crate isn't there
        let mut hallucinated = puzzle.clone();
        hallucinated[start] = Tile::Floor;

        let mut prev_moves = HashMap::new();
        prev_moves.insert(start, None);
        let mut new_moves = vec![start];

        while !new_moves.is_empty() {
            let mut last_moves = Vec::new();
            core::mem::swap(&mut new_moves, &mut last_moves);
            for prev in last_moves {
                let player = prev_moves
                    .get(&prev)
                    .unwrap()
                    .map(|direction| opposite(direction).go(prev).unwrap())
                    .unwrap_or(puzzle.player());
                if push_local(
                    player,
                    prev,
                    destination,
                    &mut hallucinated,
                    &mut prev_moves,
                    &mut new_moves,
                ) {
                    let mut crate_moves = VecDeque::new();
                    let mut next = destination;
                    // walk backwards through the flood-fill
                    while let Some(&Some(direction)) = prev_moves.get(&next) {
                        next = opposite(direction).go(next).unwrap();
                        crate_moves.push_front(direction);
                    }

                    hallucinated[start] = Tile::Crate;

                    let mut assembled = Vec::new();
                    let mut last_executed = 0;
                    let mut last_position = start;
                    for &next_move in crate_moves.iter() {
                        // execute the player moves that we haven't done yet
                        hallucinated = assembled[last_executed..]
                            .iter()
                            .try_fold(hallucinated, |puzzle, &direction| {
                                puzzle.move_player(direction)
                            })
                            .unwrap();
                        last_executed = assembled.len();

                        // queue the moves to get the player to the push point
                        if let Some(path) = go_to(
                            hallucinated.player(),
                            opposite(next_move).go(last_position).unwrap(),
                            &hallucinated,
                        ) {
                            assembled.extend(path);
                        } else {
                            eprintln!("while attempting to apply {crate_moves:?} to {puzzle:?}");
                            panic!("unable to queue movement {next_move:?} for box at {last_position:?}: {hallucinated:?} (player at {:?})", hallucinated.player());
                        }
                        // queue the moves to push the box
                        assembled.push(next_move);
                        last_position = next_move.go(last_position).unwrap();
                    }

                    return Some(assembled);
                }
            }
        }
    }
    None
}

pub fn hash_sokoban_state(state: &SokobanState, include_player: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    for item in state.iter().filter(|item| item.tile() == Tile::Crate) {
        item.position().hash(&mut hasher);
    }
    if include_player {
        state.player().hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod test {
    use crate::util::{go_to, push_to};
    use sokoban::Direction::{Right, Up};
    use sokoban::{State as SokobanState, Tile};

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

        let moves =
            go_to(puzzle.player(), (15, 3), &puzzle).expect("Couldn't find path to (15, 3)!");
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

        let moves = go_to(puzzle.player(), (3, 3), &puzzle).expect("Couldn't find path to (3, 3)!");
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

        assert!(go_to(puzzle.player(), (3, 3), &puzzle).is_none());
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

        assert!(go_to(puzzle.player(), (0, 0), &puzzle).is_none());
    }

    #[test]
    fn test_push_to_simple() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#__________________#
#__________________#
#______________m___#
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

        let moves = push_to(
            Up.go(Right.go(puzzle.player()).unwrap()).unwrap(),
            (15, 3),
            &puzzle,
        )
        .expect("Couldn't find path to (15, 3)!");
        println!("{:?}", moves);
        let puzzle = moves
            .into_iter()
            .try_fold(puzzle, |puzzle, direction| puzzle.move_player(direction))
            .expect("Should not make invalid moves!");

        assert_eq!(puzzle[(15, 3)], Tile::Crate);
    }

    #[test]
    fn test_path_to_around_wall() {
        let puzzle = SokobanState::parse(
            &br#"
####################
#________#_________#
#________#_____m___#
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
#__________________#
#__________________#
####################
"#[..],
        )
        .unwrap();

        let moves = push_to(
            Up.go(Right.go(puzzle.player()).unwrap()).unwrap(),
            (3, 3),
            &puzzle,
        )
        .expect("Couldn't find path to (3, 3)!");
        println!("{:?}", moves);
        let puzzle = moves
            .into_iter()
            .try_fold(puzzle, |puzzle, direction| puzzle.move_player(direction))
            .expect("Should not make invalid moves!");

        assert_eq!(puzzle[(3, 3)], Tile::Crate);
    }
}
