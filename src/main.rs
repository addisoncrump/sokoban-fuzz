use crate::executor::SokobanExecutor;
use crate::feedback::{SokobanSolvableFeedback, SokobanSolvedFeedback, SokobanStatisticsFeedback};
use crate::input::SokobanInput;
use crate::mutators::{MoveCrateMutator, MoveCrateToTargetMutator, RandomPreferenceMutator};
use crate::observer::SokobanStateObserver;
use crate::scheduler::SokobanWeightScheduler;
use crate::state::InitialPuzzleMetadata;
use libafl::corpus::{Corpus, CorpusId, HasTestcase, InMemoryCorpus};
use libafl::events::Event::UpdateUserStats;
use libafl::events::{EventFirer, SimpleEventManager};
use libafl::feedbacks::NewHashFeedback;
use libafl::monitors::UserStats;

use libafl::prelude::{feedback_and_fast, tuple_list, MultiMonitor, RandomSeed, StdRand};
use libafl::stages::StdMutationalStage;
use libafl::state::{HasMetadata, HasSolutions, StdState};
use libafl::{Error, Evaluator, Fuzzer, StdFuzzer};
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use sokoban::{State as SokobanState, Tile};

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

    for level in 20..100 {
        let response: Response = from_str(
            &reqwest::blocking::get(format!(
                "http://www.linusakesson.net/games/autosokoban/board.php?v=1&seed={}&level={}",
                seed, level
            ))?
            .text()?,
        )?;

        let puzzle = SokobanState::from(response);

        println!("level {level}");
        fuzz(level, puzzle)?;
    }

    Ok(())
}

fn fuzz(level: u64, puzzle: SokobanState) -> Result<(), Error> {
    println!("starting state: {:?}", puzzle);

    #[cfg(feature = "printing")]
    let print_fn = |s| println!("{s}");
    #[cfg(not(feature = "printing"))]
    let print_fn = |_| {};
    let monitor = MultiMonitor::new(print_fn);

    let mut mgr = SimpleEventManager::new(monitor);

    let sokoban_obs = SokobanStateObserver::new("sokoban_state", false);

    let scheduler = SokobanWeightScheduler::new(&sokoban_obs);

    let mut feedback = feedback_and_fast!(
        SokobanSolvableFeedback::new(&sokoban_obs),
        NewHashFeedback::new(&sokoban_obs),
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

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let _ = fuzzer.evaluate_input(
        &mut state,
        &mut executor,
        &mut mgr,
        SokobanInput::new(Vec::new()),
    )?;

    let mutator =
        RandomPreferenceMutator::new(tuple_list!(MoveCrateMutator, MoveCrateToTargetMutator));
    let mutational_stage = StdMutationalStage::transforming(mutator);

    let mut stages = tuple_list!(mutational_stage);

    mgr.fire(
        &mut state,
        UpdateUserStats {
            name: "level".to_string(),
            value: UserStats::Number(level),
            phantom: Default::default(),
        },
    )?;

    while state.solutions().is_empty() {
        fuzzer.fuzz_one(&mut stages, &mut executor, &mut state, &mut mgr)?;
    }

    let testcase = state.solutions().testcase(CorpusId::from(0u64))?;
    let input = testcase.input().as_ref().unwrap().clone();

    let final_state = input
        .moves()
        .iter()
        .cloned()
        .try_fold(puzzle.clone(), |puzzle, direction| {
            puzzle.move_player(direction)
        })
        .unwrap();

    println!("moves: {:?}", input.moves());
    println!("final state: {:?}", final_state);

    Ok(())
}
