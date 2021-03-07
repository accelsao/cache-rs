use cht::HashMap;
use crate::ConcurrentCache;
use std::sync::Arc;

pub struct Cache<K, V> {
    store: HashMap<K, V>,
}

impl<K, V> Cache<K, V> {
    pub fn new() -> Self {
        Self {
            store: HashMap::default(),
        }
    }
}

impl<K,V> ConcurrentCache<K,V> for Cache<K,V> {
    fn get(&self, key: &K) -> Option<Arc<V>> {
        unimplemented!()
    }

    fn get_or_insert(&self, key: K, default: V) -> Arc<V> {
        unimplemented!()
    }

    fn get_or_insert_with<F>(&self, key: K, default: F) -> Arc<V> where
        F: FnOnce() -> V {
        unimplemented!()
    }

    fn insert(&self, key: K, value: V) {
        unimplemented!()
    }

    fn remove(&self, key: &K) -> Option<Arc<V>> {
        unimplemented!()
    }
}