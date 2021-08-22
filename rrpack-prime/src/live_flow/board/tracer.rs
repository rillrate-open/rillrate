use super::state::*;
use crate::auto_path::AutoPath;
use crate::manifest::Binder;
use derive_more::{Deref, DerefMut};
use rill_engine::tracers::tracer::Tracer;
use rill_protocol::flow::core::FlowMode;

#[derive(Debug, Deref, DerefMut, Clone)]
pub struct Board {
    #[deref]
    #[deref_mut]
    tracer: Tracer<BoardState>,
    _binder: Binder,
}

impl Board {
    pub fn new(auto_path: impl Into<AutoPath>, mode: FlowMode, spec: BoardSpec) -> Self {
        let path = auto_path.into();
        let state = spec.into();
        let tracer = Tracer::new(state, path.into(), mode);
        let binder = Binder::new(&tracer);
        Self {
            tracer,
            _binder: binder,
        }
    }

    pub fn set(&self, key: impl ToString, value: impl ToString) {
        let msg = BoardEvent::Assign {
            key: key.to_string(),
            value: value.to_string(),
        };
        self.tracer.send(msg, None);
    }

    pub fn remove(&self, key: impl ToString) {
        let msg = BoardEvent::Remove {
            key: key.to_string(),
        };
        self.tracer.send(msg, None);
    }
}
