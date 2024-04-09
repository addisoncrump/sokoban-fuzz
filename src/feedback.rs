use crate::input::SokobanInput;
use crate::observer::SokobanStateObserver;
use crate::util::find_crates;
use libafl::events::{Event, EventFirer};
use libafl::executors::ExitKind;
use libafl::feedbacks::Feedback;
use libafl::monitors::{UserStats, UserStatsValue};
use libafl::observers::ObserversTuple;
use libafl::prelude::AggregatorOps;
use libafl::state::State;
use libafl::Error;
use libafl_bolts::Named;
use sokoban::Direction::{Down, Left, Right, Up};
use sokoban::Tile;

#[derive(Debug)]
pub struct SokobanSolvedFeedback {
    obs_name: String,
    name: String,
}

impl SokobanSolvedFeedback {
    pub fn new(obs: &SokobanStateObserver) -> Self {
        Self {
            obs_name: obs.name().to_string(),
            name: format!("solved_{}", obs.name()),
        }
    }
}

impl Named for SokobanSolvedFeedback {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<S> Feedback<S> for SokobanSolvedFeedback
where
    S: State,
{
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        let state_obs = observers
            .match_name::<SokobanStateObserver>(&self.obs_name)
            .unwrap();

        if let Some(last_state) = state_obs.last_state() {
            Ok(last_state.in_solution_state())
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug)]
pub struct SokobanSolvableFeedback {
    obs_name: String,
    name: String,
}

impl SokobanSolvableFeedback {
    pub fn new(obs: &SokobanStateObserver) -> Self {
        Self {
            obs_name: obs.name().to_string(),
            name: format!("cornered_{}", obs.name()),
        }
    }
}

impl Named for SokobanSolvableFeedback {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<S> Feedback<S> for SokobanSolvableFeedback
where
    S: State,
{
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        let state_obs = observers
            .match_name::<SokobanStateObserver>(&self.obs_name)
            .unwrap();

        if let Some(last_state) = state_obs.last_state() {
            let crates = find_crates(last_state);
            for maybe_cornered in crates {
                if !last_state.targets().contains(&maybe_cornered) {
                    // we assume we are within the appropriate bounds
                    if let Some(above) = Up.go(maybe_cornered) {
                        if last_state[above] == Tile::Wall {
                            if let Some(left) = Left.go(maybe_cornered) {
                                if last_state[left] == Tile::Wall {
                                    return Ok(false);
                                }
                            }
                            if let Some(right) = Right.go(maybe_cornered) {
                                if last_state[right] == Tile::Wall {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                    if let Some(below) = Down.go(maybe_cornered) {
                        if last_state[below] == Tile::Wall {
                            if let Some(left) = Left.go(maybe_cornered) {
                                if last_state[left] == Tile::Wall {
                                    return Ok(false);
                                }
                            }
                            if let Some(right) = Right.go(maybe_cornered) {
                                if last_state[right] == Tile::Wall {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug)]
pub struct SokobanStatisticsFeedback {
    most_set: usize,
    most_moves: usize,
    obs_name: String,
    name: String,
}

impl SokobanStatisticsFeedback {
    pub fn new(obs: &SokobanStateObserver) -> Self {
        Self {
            most_set: 0,
            most_moves: 0,
            obs_name: obs.name().to_string(),
            name: format!("stats_{}", obs.name()),
        }
    }
}

impl Named for SokobanStatisticsFeedback {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<S> Feedback<S> for SokobanStatisticsFeedback
where
    S: State<Input = SokobanInput>,
{
    fn is_interesting<EM, OT>(
        &mut self,
        state: &mut S,
        manager: &mut EM,
        input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        let state_obs = observers
            .match_name::<SokobanStateObserver>(&self.obs_name)
            .unwrap();

        if let Some(last_state) = state_obs.last_state() {
            let most_set = last_state
                .targets()
                .iter()
                .filter(|&&target| last_state[target] == Tile::Crate)
                .count();
            if most_set > self.most_set {
                manager.fire(
                    state,
                    Event::UpdateUserStats {
                        name: "most_set".to_string(),
                        value: UserStats::new(
                            UserStatsValue::Ratio(
                                most_set as u64,
                                last_state.targets().len() as u64,
                            ),
                            AggregatorOps::Max,
                        ),
                        phantom: Default::default(),
                    },
                )?;
                self.most_set = most_set;
            }
            if input.moves().len() > self.most_moves {
                manager.fire(
                    state,
                    Event::UpdateUserStats {
                        name: "most_moves".to_string(),
                        value: UserStats::new(
                            UserStatsValue::Number(input.moves().len() as u64),
                            AggregatorOps::Max,
                        ),
                        phantom: Default::default(),
                    },
                )?;
                self.most_moves = input.moves().len();
            }
        }
        Ok(true)
    }
}
