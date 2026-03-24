use serde::{de::DeserializeOwned, Serialize};

use crate::unified::UnifiedEvent;

/// Generic batch accumulator. Collects unified events and flushes
/// when the batch reaches capacity. Generic over the original source
/// type to force monomorphization.
pub struct Batch<T: Serialize + DeserializeOwned> {
    events: Vec<UnifiedEvent>,
    capacity: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> Batch<T> {
    pub fn new(capacity: usize) -> Self {
        Batch {
            events: Vec::with_capacity(capacity),
            capacity,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add an event to the batch. Returns true if the batch is now full.
    pub fn add(&mut self, event: UnifiedEvent) -> bool {
        self.events.push(event);
        self.is_full()
    }

    /// Drain and return all events in the batch.
    pub fn flush(&mut self) -> Vec<UnifiedEvent> {
        std::mem::take(&mut self.events)
    }

    /// Check if the batch has reached capacity.
    pub fn is_full(&self) -> bool {
        self.events.len() >= self.capacity
    }

    /// Current number of events in the batch.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
