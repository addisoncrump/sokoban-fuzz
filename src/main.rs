use crate::executor::SokobanExecutor;
use crate::feedback::{SokobanSolvableFeedback, SokobanSolvedFeedback, SokobanStatisticsFeedback};
use crate::input::SokobanInput;
use crate::mutators::{MoveCrateMutator, MoveCrateToTargetMutator, OneShotMutator};
use crate::observer::SokobanStateObserver;
use crate::scheduler::SokobanWeightScheduler;
use crate::state::{InitialPuzzleMetadata, LastHallucinationMetadata};
use libafl::corpus::HasTestcase;
use libafl::prelude::SimpleMonitor;
use libafl::{
    corpus::{Corpus, InMemoryCorpus},
    events::Event::{Objective, UpdateUserStats},
    events::{EventFirer, SimpleEventManager},
    feedbacks::NewHashFeedback,
    monitors::{
        Monitor,
        // MultiMonitor,
        UserStats,
    },
    prelude::{feedback_and_fast, tuple_list, RandomSeed, RomuDuoJrRand, StdRand},
    stages::StdMutationalStage,
    state::{HasMetadata, HasSolutions, StdState},
    Error, Evaluator, Fuzzer, StdFuzzer,
};
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use sokoban::{State as SokobanState, Tile};
use std::str::FromStr;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seed = 0;

    let initial = std::env::args()
        .nth(1)
        .and_then(|s| u64::from_str(&s).ok())
        .unwrap_or(1);

    // let monitor = TuiMonitor::new(TuiUI::new("sokoban-fuzz".to_string(), true));
    let monitor = SimpleMonitor::new(|_| {});

    let mut mgr = SimpleEventManager::new(monitor);

    for level in initial..=100 {
        let response: Response = from_str(
            &reqwest::blocking::get(format!(
                "http://www.linusakesson.net/games/autosokoban/board.php?v=1&seed={}&level={}",
                seed, level
            ))?
            .text()?,
        )?;

        let puzzle = SokobanState::from(response);

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
            value: UserStats::Number(level),
            phantom: Default::default(),
        },
    )?;
    mgr.fire(&mut state, Objective { objective_size: 0 })?;

    while state.solutions().is_empty() {
        fuzzer.fuzz_one(&mut stages, &mut executor, &mut state, mgr)?;
    }

    let solution = state.solutions().first().unwrap();
    let mut testcase = state.solutions().testcase_mut(solution)?;
    let moves = testcase.load_input(state.solutions())?;

    println!("moves: {:?}", moves.moves());

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
