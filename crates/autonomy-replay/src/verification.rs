use autonomy_sim::WorldState;

use crate::{
    event_log::EventEnvelope,
    replay::{replay_events, ReplayError},
};

pub fn verify_replay(
    initial_state: &WorldState,
    events: &[EventEnvelope],
    expected_final_state: &WorldState,
) -> Result<(), ReplayError> {
    let replayed = replay_events(initial_state, events)?;

    if &replayed != expected_final_state {
        return Err(ReplayError::FinalStateMismatch);
    }

    Ok(())
}
