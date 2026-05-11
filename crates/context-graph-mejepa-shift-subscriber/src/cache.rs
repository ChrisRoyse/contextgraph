// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use context_graph_mejepa_instruments::InstrumentSlot;

use crate::{Result, SubscriberError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlotKey {
    pub source_sha256: [u8; 32],
    pub slot: InstrumentSlot,
}

pub struct InstrumentCache {
    budget_bytes: u64,
    entries: RwLock<HashMap<SlotKey, Vec<f32>>>,
    order: RwLock<VecDeque<SlotKey>>,
    ram_used_bytes: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl InstrumentCache {
    pub fn new(budget_bytes: u64) -> Result<Self> {
        if budget_bytes == 0 {
            return Err(SubscriberError::invalid(
                "instrument_cache.budget_bytes",
                "budget_bytes must be > 0",
            ));
        }
        Ok(Self {
            budget_bytes,
            entries: RwLock::new(HashMap::new()),
            order: RwLock::new(VecDeque::new()),
            ram_used_bytes: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        })
    }

    pub fn get_or_insert_with<F>(&self, key: SlotKey, compute: F) -> Result<Vec<f32>>
    where
        F: FnOnce() -> Result<Vec<f32>>,
    {
        if let Some(value) = self.entries.read().expect("cache lock").get(&key).cloned() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            self.touch(&key);
            return Ok(value);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let value = compute()?;
        let bytes = value
            .len()
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| SubscriberError::invalid("instrument_cache.value", "byte overflow"))?
            as u64;
        if bytes > self.budget_bytes {
            return Err(SubscriberError::InstrumentCacheOverflow {
                requested_bytes: bytes,
                budget_bytes: self.budget_bytes,
            });
        }
        {
            let mut entries = self.entries.write().expect("cache lock");
            if let Some(existing) = entries.get(&key) {
                return Ok(existing.clone());
            }
            while self.ram_used_bytes.load(Ordering::Relaxed) + bytes > self.budget_bytes {
                let victim = self
                    .order
                    .write()
                    .expect("cache lock")
                    .pop_front()
                    .ok_or_else(|| SubscriberError::InstrumentCacheOverflow {
                        requested_bytes: bytes,
                        budget_bytes: self.budget_bytes,
                    })?;
                if let Some(removed) = entries.remove(&victim) {
                    let removed_bytes = (removed.len() * std::mem::size_of::<f32>()) as u64;
                    self.ram_used_bytes
                        .fetch_sub(removed_bytes, Ordering::Relaxed);
                }
            }
            entries.insert(key.clone(), value.clone());
        }
        self.order.write().expect("cache lock").push_back(key);
        self.ram_used_bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(value)
    }

    pub fn entries(&self) -> usize {
        self.entries.read().expect("cache lock").len()
    }

    pub fn ram_used_bytes(&self) -> u64 {
        self.ram_used_bytes.load(Ordering::Relaxed)
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let total = hits + self.misses.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    fn touch(&self, key: &SlotKey) {
        let mut order = self.order.write().expect("cache lock");
        if let Some(pos) = order.iter().position(|candidate| candidate == key) {
            order.remove(pos);
        }
        order.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_evicts_and_rejects_oversized_insert() {
        let cache = InstrumentCache::new(16).unwrap();
        for idx in 0..5u8 {
            let key = SlotKey {
                source_sha256: [idx; 32],
                slot: InstrumentSlot::EOracle,
            };
            cache
                .get_or_insert_with(key, || Ok(vec![idx as f32; 4]))
                .unwrap();
            assert!(cache.ram_used_bytes() <= 16);
        }
        assert_eq!(cache.entries(), 1);
        let oversized = SlotKey {
            source_sha256: [9; 32],
            slot: InstrumentSlot::EOracle,
        };
        assert_eq!(
            cache
                .get_or_insert_with(oversized, || Ok(vec![0.0; 5]))
                .unwrap_err()
                .code(),
            "MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW"
        );
    }
}
