use autonomy_core::{EventId, SimError, Tick};
use autonomy_sim::{apply_action, WorkerAction, WorldState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope {
    pub id: EventId,
    pub tick: Tick,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    ActionRequested {
        action: WorkerAction,
    },
    ActionApplied {
        action: WorkerAction,
        resulting_tick: Tick,
    },
    ActionRejected {
        action: WorkerAction,
        error: SimError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLog {
    events: Vec<EventEnvelope>,
    next_id: EventId,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_id: EventId::new(1),
        }
    }

    pub fn append(&mut self, tick: Tick, kind: EventKind) -> EventEnvelope {
        let envelope = EventEnvelope {
            id: self.next_id,
            tick,
            kind,
        };
        self.next_id = self
            .next_id
            .checked_next()
            .expect("expect id overflow while appending to event log");
        self.events.push(envelope.clone());
        envelope
    }

    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn record_action(
    state: &WorldState,
    log: &mut EventLog,
    action: WorkerAction,
) -> Result<WorldState, SimError> {
    let pre_action_tick = state.tick;
    log.append(
        pre_action_tick,
        EventKind::ActionRequested {
            action: action.clone(),
        },
    );

    match apply_action(state, &action) {
        Ok(next_state) => {
            let resulting_tick = next_state.tick;
            log.append(
                resulting_tick,
                EventKind::ActionApplied {
                    action,
                    resulting_tick,
                },
            );
            Ok(next_state)
        }
        Err(error) => {
            log.append(
                pre_action_tick,
                EventKind::ActionRejected {
                    action,
                    error: error.clone(),
                },
            );
            Err(error)
        }
    }
}
