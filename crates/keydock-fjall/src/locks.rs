use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, MutexGuard};

use crate::FjallError;

const WRITE_LOCK_STRIPES: usize = 64;

pub(crate) struct StripedLocks {
    locks: [Mutex<()>; WRITE_LOCK_STRIPES],
}

impl StripedLocks {
    pub(crate) fn new() -> Self {
        Self {
            locks: std::array::from_fn(|_| Mutex::new(())),
        }
    }

    pub(crate) fn stripe_index(storage_key: &[u8]) -> usize {
        let mut h = DefaultHasher::new();
        storage_key.hash(&mut h);
        (h.finish() as usize) % WRITE_LOCK_STRIPES
    }

    pub(crate) fn lock_for(&self, storage_key: &[u8]) -> Result<MutexGuard<'_, ()>, FjallError> {
        let idx = Self::stripe_index(storage_key);
        self.locks[idx]
            .lock()
            .map_err(|_| FjallError::Adapter("write lock poisoned".into()))
    }

    pub(crate) fn lock_stripes_for_keys<'a, 'k>(
        &'a self,
        storage_keys: impl Iterator<Item = &'k [u8]>,
    ) -> Result<Vec<MutexGuard<'a, ()>>, FjallError> {
        let mut stripes: Vec<usize> = storage_keys.map(Self::stripe_index).collect();
        stripes.sort_unstable();
        stripes.dedup();

        let mut guards = Vec::with_capacity(stripes.len());
        for idx in stripes {
            let g = self.locks[idx]
                .lock()
                .map_err(|_| FjallError::Adapter("write lock poisoned".into()))?;
            guards.push(g);
        }
        Ok(guards)
    }
}
