use std::collections::VecDeque;

/// Simple LRU snapshot cache keyed by step number.
pub struct SnapshotCache<T> {
    capacity: usize,
    entries: VecDeque<(u64, T)>,
}

impl<T: Clone> SnapshotCache<T> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, entries: VecDeque::with_capacity(capacity) }
    }

    pub fn insert(&mut self, step: u64, snapshot: T) {
        // Evict oldest if full
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((step, snapshot));
    }

    pub fn get(&self, step: u64) -> Option<&T> {
        self.entries.iter().find(|(s, _)| *s == step).map(|(_, t)| t)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
