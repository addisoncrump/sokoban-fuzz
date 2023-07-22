use crate::input::SokobanInput;
use crate::observer::{SokobanObserversTuple, SokobanStateObserver};
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl::inputs::UsesInput;
use libafl::observers::{ObserversTuple, UsesObservers};
use libafl::state::UsesState;
use libafl::Error;
use sokoban::State as SokobanState;
use std::fmt::Debug;
use std::marker::PhantomData;

#[derive(Debug)]
pub struct SokobanExecutor<OT, S> {
    initial: SokobanState,
    observers: OT,
    state_observer_name: String,
    phantom: PhantomData<S>,
}

impl<OT, S> SokobanExecutor<OT, S>
where
    OT: SokobanObserversTuple,
{
    pub fn new(initial: SokobanState, observers: OT) -> Self {
        Self {
            initial,
            state_observer_name: observers.sokoban_observer_name().to_string(),
            observers,
            phantom: PhantomData,
        }
    }
}

impl<OT, S> UsesState for SokobanExecutor<OT, S>
where
    S: UsesInput<Input = SokobanInput>,
{
    type State = S;
}

impl<EM, OT, S, Z> Executor<EM, Z> for SokobanExecutor<OT, S>
where
    EM: UsesState<State = Self::State>,
    OT: ObserversTuple<S> + SokobanObserversTuple + Debug,
    S: UsesInput<Input = SokobanInput> + Debug,
    Z: UsesState<State = Self::State>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        _state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<ExitKind, Error> {
        if let Ok(state) = input
            .moves()
            .iter()
            .cloned()
            .try_fold(self.initial.clone(), |state, dir| state.move_player(dir))
        {
            let sokoban_observer = self
                .observers
                .match_name_mut::<SokobanStateObserver>(&self.state_observer_name)
                .unwrap();
            sokoban_observer.replace(state);
            Ok(ExitKind::Ok)
        } else {
            Ok(ExitKind::Crash)
        }
    }
}

impl<OT, S> UsesObservers for SokobanExecutor<OT, S>
where
    OT: ObserversTuple<Self::State>,
    S: UsesInput<Input = SokobanInput>,
{
    type Observers = OT;
}

impl<OT, S> HasObservers for SokobanExecutor<OT, S>
where
    OT: ObserversTuple<Self::State>,
    S: UsesInput<Input = SokobanInput>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
