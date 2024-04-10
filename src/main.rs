use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use libafl::corpus::HasTestcase;
use libafl::monitors::{AggregatorOps, UserStatsValue};
use libafl::{
    corpus::{Corpus, InMemoryCorpus},
    events::Event::{Objective, UpdateUserStats},
    events::{EventFirer, SimpleEventManager},
    feedback_and_fast,
    feedbacks::NewHashFeedback,
    monitors::{Monitor, SimpleMonitor, UserStats},
    stages::StdMutationalStage,
    state::{HasCorpus, HasMaxSize, HasMetadata, HasSolutions, StdState},
    Error, Evaluator, Fuzzer, StdFuzzer,
};
use libafl_bolts::rands::{RandomSeed, RomuDuoJrRand, StdRand};
use libafl_bolts::tuples::tuple_list;
use serde::{Deserialize, Serialize};
use sokoban::{State as SokobanState, Tile};

use crate::executor::SokobanExecutor;
use crate::feedback::{SokobanSolvableFeedback, SokobanSolvedFeedback, SokobanStatisticsFeedback};
use crate::input::SokobanInput;
use crate::mutators::{MoveCrateMutator, MoveCrateToTargetMutator, OneShotMutator};
use crate::observer::SokobanStateObserver;
use crate::scheduler::SokobanWeightScheduler;
use crate::state::{InitialPuzzleMetadata, LastHallucinationMetadata};

mod executor;
mod feedback;
mod input;
mod mutators;
mod observer;
mod scheduler;
mod state;
mod util;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Response {
    deal: String,
}

impl From<Response> for SokobanState {
    fn from(resp: Response) -> Self {
        let dim_r = 12;
        let dim_c = 18;
        let container = resp
            .deal
            .chars()
            .map(|c| match c {
                'w' => Tile::Wall,
                'e' | 'E' | 'm' | 'M' => Tile::Floor,
                'o' | 'O' => Tile::Crate,
                _ => unreachable!("Illegal value in response."),
            })
            .collect::<Vec<_>>();
        let player = resp
            .deal
            .char_indices()
            .find(|(_, c)| *c == 'm' || *c == 'M')
            .expect("Couldn't find the player")
            .0;

        let player = (player / dim_c, player % dim_c);
        let targets = resp
            .deal
            .char_indices()
            .filter_map(|(i, c)| c.is_ascii_uppercase().then_some(i))
            .map(|i| (i / dim_c, i % dim_c))
            .collect::<Vec<_>>();
        SokobanState::new(container, player, targets, dim_r, dim_c)
            .expect("Expected a valid state from remote")
    }
}

fn parse_file(file: File) -> Result<SokobanState, std::io::Error> {
    let reader = BufReader::new(file);

    let mut dim_c = 0;
    let mut dim_r = 0;

    let mut rows = Vec::new();

    for line in reader.lines() {
        dim_r += 1;
        let line = line?.into_bytes();
        dim_c = dim_c.max(line.len());
        rows.push(line);
    }

    for row in &mut rows {
        let needed = dim_c - row.len();
        row.extend(std::iter::repeat(b' ').take(needed));
    }

    let raw = rows.into_iter().flatten().collect::<Vec<_>>();

    let container = raw
        .iter()
        .copied()
        .map(|b| match b {
            b' ' | b'@' | b'.' => Tile::Floor,
            b'#' => Tile::Wall,
            b'$' => Tile::Crate,
            _ => unreachable!("Illegal value in response."),
        })
        .collect::<Vec<_>>();

    let player = raw
        .iter()
        .copied()
        .enumerate()
        .find(|&(_, c)| c == b'@')
        .expect("Couldn't find the player")
        .0;
    let player = (player / dim_c, player % dim_c);

    let targets = raw
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(i, c)| (c == b'.').then_some(i))
        .map(|i| (i / dim_c, i % dim_c))
        .collect::<Vec<_>>();

    Ok(SokobanState::new(container, player, targets, dim_r, dim_c)
        .expect("Expected a valid state from file"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let initial = std::env::args()
        .nth(1)
        .and_then(|s| u64::from_str(&s).ok())
        .unwrap_or(1);

    let end = std::env::args()
        .nth(2)
        .and_then(|s| u64::from_str(&s).ok())
        .unwrap_or(initial);

    // let monitor = TuiMonitor::new(TuiUI::new("sokoban-fuzz".to_string(), true));
    let monitor = SimpleMonitor::new(|_| {});
    // let monitor = SimplePrintingMonitor::new();

    let mut mgr = SimpleEventManager::new(monitor);

    for level in initial..=end {
        // let response: Response = from_str(
        //     &reqwest::blocking::get(format!(
        //         "http://www.linusakesson.net/games/autosokoban/board.php?v=1&seed={}&level={}",
        //         seed, level
        //     ))?
        //     .text()?,
        // )?;
        //
        // let puzzle = SokobanState::from(response);

        let puzzle = parse_file(File::open(format!("puzzles/screen.{level}"))?)?;

        if let Err(e) = fuzz(&mut mgr, level, puzzle) {
            eprintln!("{e}");
            return Ok(());
        }
    }

    Ok(())
}

type SokobanManager<M> = SimpleEventManager<
    M,
    StdState<
        SokobanInput,
        InMemoryCorpus<SokobanInput>,
        RomuDuoJrRand,
        InMemoryCorpus<SokobanInput>,
    >,
>;

fn fuzz(
    mgr: &mut SokobanManager<impl Monitor>,
    level: u64,
    puzzle: SokobanState,
) -> Result<(), Error> {
    println!("puzzle {level}: {puzzle:?}");

    let sokoban_obs = SokobanStateObserver::new("sokoban_state", true);

    let mut feedback = feedback_and_fast!(
        NewHashFeedback::new(&sokoban_obs),
        SokobanSolvableFeedback::new(&sokoban_obs),
        SokobanStatisticsFeedback::new(&sokoban_obs)
    );
    let mut objective = SokobanSolvedFeedback::new(&sokoban_obs);

    let observers = tuple_list!(sokoban_obs);
    let mut executor = SokobanExecutor::new(puzzle.clone(), observers);

    let mut state = StdState::new(
        StdRand::new(),
        InMemoryCorpus::new(),
        InMemoryCorpus::new(),
        &mut feedback,
        &mut objective,
    )?;

    state.add_metadata(InitialPuzzleMetadata::new(puzzle.clone()));
    state.add_metadata(LastHallucinationMetadata::default());

    let scheduler = SokobanWeightScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let _ = fuzzer.evaluate_input(
        &mut state,
        &mut executor,
        mgr,
        SokobanInput::new(Vec::new()),
    )?;

    let oneshot_stage = StdMutationalStage::transforming(OneShotMutator);
    let move_stage = StdMutationalStage::transforming(MoveCrateMutator);
    let move_to_target_stage = StdMutationalStage::transforming(MoveCrateToTargetMutator);

    let mut stages = tuple_list!(oneshot_stage, move_stage, move_to_target_stage);

    mgr.fire(
        &mut state,
        UpdateUserStats {
            name: "level".to_string(),
            value: UserStats::new(UserStatsValue::Number(level), AggregatorOps::Max),
            phantom: Default::default(),
        },
    )?;
    mgr.fire(&mut state, Objective { objective_size: 0 })?;

    while state.solutions().is_empty() {
        let _ = match fuzzer.fuzz_one(&mut stages, &mut executor, &mut state, mgr) {
            Err(Error::KeyNotFound(s, _bt))
                if !state.solutions().is_empty()
                    && s.starts_with("Missing corpus entry; is the corpus empty?") =>
            {
                // we found a solution at the exact same time we cleared to zero corpus entries
                break;
            }
            r => r?,
        };
    }

    let mut smallest_id = state.solutions().first().unwrap();
    let mut testcase = state.solutions().testcase_mut(smallest_id)?;
    let moves = testcase.load_input(state.solutions())?;

    println!("first solution: {:?}", moves.moves());
    /*    drop(testcase);

        let move_stage = StdMutationalStage::transforming(MoveCrateMutator);
        let move_to_target_stage = StdMutationalStage::transforming(MoveCrateToTargetMutator);

        // oneshot is no longer worthwhile, as it poisons our minimisation
        let mut stages = tuple_list!(move_stage, move_to_target_stage);

        loop {
            let mut smallest_len = usize::MAX;
            for id in state.solutions().ids() {
                let mut testcase = state.solutions().testcase_mut(id)?;
                let input = testcase.load_input(state.solutions())?;
                if input.moves().len() < smallest_len {
                    smallest_len = input.moves().len();
                    smallest_id = id;
                }
            }

            state.set_max_size(smallest_len);

            if state.corpus().is_empty() {
                break;
            }

            let _ = match fuzzer.fuzz_one(&mut stages, &mut executor, &mut state, mgr) {
                Err(Error::KeyNotFound(s, _bt))
                    if s.starts_with("Missing corpus entry; is the corpus empty?") =>
                {
                    // we found a solution at the exact same time we cleared to zero corpus entries
                    continue;
                }
                r => r?,
            };
        }

        let mut testcase = state.solutions().testcase_mut(smallest_id)?;
        let moves = testcase.load_input(state.solutions())?;

        println!("minimised: {:?}", moves.moves());
    */
    let solution = moves
        .moves()
        .iter()
        .copied()
        .try_fold(puzzle.clone(), |puzzle, direction| {
            puzzle.move_player(direction)
        })
        .unwrap();

    println!("solved: {solution:?}");

    Ok(())
}
