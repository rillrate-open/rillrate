use super::{Flow, TimedEvent};
use crate::io::provider::{StreamType, Timestamp};
use crate::range::Range;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GaugeFlow {
    pub range: Range,
}

impl Flow for GaugeFlow {
    type State = GaugeState;
    type Event = GaugeEvent;

    fn stream_type() -> StreamType {
        StreamType::from("rillrate.gauge.v0")
    }

    fn apply(&self, state: &mut Self::State, event: TimedEvent<Self::Event>) {
        match event.event {
            GaugeEvent::Set(delta) => {
                state.timestamp = Some(event.timestamp);
                state.value = delta;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeState {
    pub timestamp: Option<Timestamp>,
    pub value: f64,
}

#[allow(clippy::new_without_default)]
impl GaugeState {
    pub fn new() -> Self {
        Self {
            timestamp: None,
            value: 0.0,
        }
    }

    pub fn last(&self) -> Option<TimedEvent<f64>> {
        self.timestamp.map(|ts| TimedEvent {
            timestamp: ts,
            event: self.value,
        })
    }
}

pub type GaugeDelta = Vec<TimedEvent<GaugeEvent>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GaugeEvent {
    Set(f64),
}
