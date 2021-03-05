use std::collections::HashMap;
use std::sync::{RwLock, Arc};


const NUM_SHARDS: usize = 256;

#[derive(Default)]
struct StoreItem<V> {
    key: u64,
    conflict: u64,
    value: V,
    expiration: i64,
}

trait Store<V> {
    fn get(key: u64, conflict: u64) -> (V, bool);
}

pub struct ShardedMap<V> {
    shards: Vec<Arc<RwLock<LockedMap<V>>>>,
}

impl<V> ShardedMap<V> {
    pub fn new() -> Self {
        Self {
            shards: vec_no_clone![Arc::new(RwLock::new(LockedMap::new())); NUM_SHARDS],
        }
    }
}

struct LockedMap<V> {
    data: HashMap<u64, StoreItem<V>>,
}

impl<V> LockedMap<V> {
    fn new() -> Self {
        Self {
            data: Default::default(),
        }
    }
}


