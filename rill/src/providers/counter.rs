use super::ProtectedProvider;
use derive_more::{Deref, DerefMut};
use rill_protocol::provider::{Path, RillData, StreamType};
use std::time::SystemTime;

/// Providers `Counter` metrics that can increments only.
#[derive(Debug, Deref, DerefMut)]
pub struct CounterProvider {
    #[deref]
    #[deref_mut]
    provider: ProtectedProvider<f64>,
}

impl CounterProvider {
    /// Creates a new provider instance.
    pub fn new(path: Path) -> Self {
        let provider = ProtectedProvider::new(path, StreamType::CounterStream, 0.0);
        Self { provider }
    }

    /// Increments value by the sepcific delta.
    pub fn inc(&self, delta: f64, timestamp: Option<SystemTime>) {
        if let Some(mut value) = self.provider.lock() {
            *value += delta;
            if self.provider.is_active() {
                let data = RillData::CounterRecord { value: *value };
                self.provider.send(data, timestamp);
            }
        }
    }
}
