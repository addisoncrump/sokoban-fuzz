use crate::observer::SokobanStateObserver;
use libafl::events::EventFirer;
use libafl::executors::ExitKind;
use libafl::feedbacks::Feedback;
use libafl::inputs::UsesInput;
use libafl::observers::ObserversTuple;
use libafl::prelude::Named;
use libafl::state::HasClientPerfMonitor;
use libafl::Error;

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
    S: UsesInput + HasClientPerfMonitor,
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
