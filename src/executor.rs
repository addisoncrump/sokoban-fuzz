use crate::input::SokobanInput;
use crate::observer::{SokobanObserversTuple, SokobanStateObserver};
use crate::state::LastHallucinationMetadata;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl::observers::{ObserversTuple, UsesObservers};
use libafl::state::{HasExecutions, HasMetadata, State, UsesState};
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
    S: State<Input = SokobanInput>,
{
    type State = S;
}

impl<EM, OT, S, Z> Executor<EM, Z> for SokobanExecutor<OT, S>
where
    EM: UsesState<State = Self::State>,
    OT: ObserversTuple<S> + SokobanObserversTuple + Debug,
    S: State<Input = SokobanInput> + HasMetadata + HasExecutions + Debug,
    Z: UsesState<State = Self::State>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<ExitKind, Error> {
        let hallucinated = state
            .metadata_mut::<LastHallucinationMetadata>()
            .ok()
            .and_then(|metadata| metadata.hallucination_mut().take());

        *state.executions_mut() += 1;

        #[cfg(debug_assertions)]
        if let Some(hallucinated) = hallucinated.as_ref() {
            debug_assert_eq!(
                hallucinated,
                &input
                    .moves()
                    .iter()
                    .cloned()
                    .try_fold(self.initial.clone(), |state, dir| state.move_player(dir))
                    .unwrap()
            );
        }

        if let Some(current) = hallucinated.or_else(|| {
            input
                .moves()
                .iter()
                .cloned()
                .try_fold(self.initial.clone(), |state, dir| state.move_player(dir))
                .ok()
        }) {
            let sokoban_observer = self
                .observers
                .match_name_mut::<SokobanStateObserver>(&self.state_observer_name)
                .unwrap();
            sokoban_observer.replace(current);
            Ok(ExitKind::Ok)
        } else {
            Ok(ExitKind::Crash)
        }
    }
}

impl<OT, S> UsesObservers for SokobanExecutor<OT, S>
where
    OT: ObserversTuple<Self::State>,
    S: State<Input = SokobanInput>,
{
    type Observers = OT;
}

impl<OT, S> HasObservers for SokobanExecutor<OT, S>
where
    OT: ObserversTuple<Self::State>,
    S: State<Input = SokobanInput>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
