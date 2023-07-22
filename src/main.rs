use crate::executor::SokobanExecutor;
use crate::feedback::SokobanSolvedFeedback;
use crate::input::SokobanInput;
use crate::mutators::{AddMoveMutator, MoveCrateMutator};
use crate::observer::SokobanStateObserver;
use crate::state::InitialPuzzleMetadata;
use libafl::corpus::{Corpus, CorpusId, HasTestcase, InMemoryCorpus};
use libafl::events::SimpleEventManager;
use libafl::feedbacks::NewHashFeedback;
use libafl::prelude::{tuple_list, RandomSeed, StdRand};
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::{HasMetadata, HasSolutions, StdState};
use libafl::{Error, Evaluator, Fuzzer, StdFuzzer};
use sokoban::State as SokobanState;

mod executor;
mod feedback;
mod input;
mod mutators;
mod observer;
mod state;

fn main() -> Result<(), Error> {
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
#____._____________#
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

    let mut mgr = SimpleEventManager::printing();

    let sokoban_obs = SokobanStateObserver::new("sokoban_state", false);
    let mut feedback = NewHashFeedback::new(&sokoban_obs);
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

    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    fuzzer.add_input(
        &mut state,
        &mut executor,
        &mut mgr,
        SokobanInput::new(Vec::new()),
    )?;

    let mutational_stage = StdMutationalStage::new(MoveCrateMutator);

    let mut stages = tuple_list!(mutational_stage);

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

    println!("starting state: {:?}", puzzle);
    println!("moves: {:?}", input.moves());
    println!("final state: {:?}", final_state);

    Ok(())
}
