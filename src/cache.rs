use crate::store::ShardedMap;

pub struct Cache<V> {
    store: ShardedMap<V>,
}

impl<V> Cache<V> {
    fn new() -> Self {
        Self {
            store: ShardedMap::new(),
        }
    }
}