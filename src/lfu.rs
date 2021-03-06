use crate::ConcurrentCache;

use crate::lfu::WriteOp::{Insert, Remove};
use count_min_sketch::CountMinSketch8;
use crossbeam_channel::{Receiver, SendError, Sender};
use parking_lot::{Mutex, MutexGuard, RwLock};
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::{collections::HashMap, hash, sync::Arc};

type Cache<K, V, S> = cht::HashMap<Arc<K>, Arc<V>, S>;
type KeySet<K> = HashSet<Arc<K>>;

const READ_LOG_SIZE: usize = 64;
const WRITE_LOG_SIZE: usize = 256;
const READ_LOG_HIGH_WATER_MARK: usize = 48; // 75% of READ_LOG_SIZE
const WRITE_LOG_HIGH_WATER_MARK: usize = 128; // 50% of WRITE_LOG_SIZE

enum WriteOp<K, V> {
    Insert(K, V),
    Remove(K),
}

#[derive(Clone)]
pub struct LFUCache<K, V, S> {
    inner: Arc<LFUInner<K, V, S>>,
    read_op_ch: Sender<K>,
    write_op_ch: Sender<WriteOp<K, V>>,
}

impl<K, V> LFUCache<K, V, RandomState>
where
    K: Clone + Eq + Hash + Debug,
{
    pub fn new(capacity: usize) -> Self {
        let (r_snd, r_rcv) = crossbeam_channel::bounded(READ_LOG_SIZE);
        let (w_snd, w_rcv) = crossbeam_channel::bounded(WRITE_LOG_SIZE);
        Self {
            inner: Arc::new(LFUInner::new(
                capacity,
                RandomState::default(),
                r_rcv,
                w_rcv,
            )),
            read_op_ch: r_snd,
            write_op_ch: w_snd,
        }
    }
}

impl<K, V, S> LFUCache<K, V, S>
where
    K: Clone + Eq + Hash + Debug,
    S: BuildHasher,
{
    pub fn new_with_hasher(capacity: usize, build_hasher: S) -> Self {
        let (r_snd, r_rcv) = crossbeam_channel::bounded(READ_LOG_SIZE);
        let (w_snd, w_rcv) = crossbeam_channel::bounded(WRITE_LOG_SIZE);
        Self {
            inner: Arc::new(LFUInner::new(capacity, build_hasher, r_rcv, w_rcv)),
            read_op_ch: r_snd,
            write_op_ch: w_snd,
        }
    }

    pub fn sync(&self) {
        let r_len = self.read_op_ch.len();
        if r_len > 0 {
            let r_lock = self.inner.reads_apply_lock.lock();
            self.inner.apply_reads(r_lock, r_len);
        }

        let l_len = self.write_op_ch.len();
        if l_len > 0 {
            let w_lock = self.inner.writes_apply_lock.lock();
            self.inner.apply_writes(w_lock, l_len);
        }
    }

    fn record_read_op(&self, key: &K) {
        let _ = self.read_op_ch.try_send(key.clone());
        self.apply_reads_if_needed();
    }

    fn schedule_insert_op(&self, key: K, value: V) -> Result<(), SendError<WriteOp<K, V>>> {
        let ch = &self.write_op_ch;
        // NOTE: This will be blocked if the channel is full.
        ch.send(WriteOp::Insert(key, value))?;
        self.apply_reads_writes_if_needed();
        Ok(())
    }

    fn schedule_remove_op(&self, key: &K) -> Result<(), SendError<WriteOp<K, V>>> {
        let ch = &self.write_op_ch;
        // TODO: Send a hash rather than the key itself so that we can avoid clone().
        // NOTE: This will be blocked if the channel is full.
        ch.send(WriteOp::Remove(key.clone()))?;
        self.apply_reads_writes_if_needed();
        Ok(())
    }

    fn apply_reads_if_needed(&self) {
        let len = self.read_op_ch.len();

        if self.should_apply_reads(len) {
            if let Some(lock) = self.inner.reads_apply_lock.try_lock() {
                self.inner.apply_reads(lock, len);
            }
        }
    }

    fn apply_reads_writes_if_needed(&self) {
        let w_len = self.write_op_ch.len();

        if self.should_apply_writes(w_len) {
            let r_len = self.read_op_ch.len();
            if let Some(r_lock) = self.inner.reads_apply_lock.try_lock() {
                self.inner.apply_reads(r_lock, r_len);
            }

            if let Some(w_lock) = self.inner.writes_apply_lock.try_lock() {
                self.inner.apply_writes(w_lock, w_len);
            }
        }
    }

    fn should_apply_reads(&self, ch_len: usize) -> bool {
        // TODO: Also check how long past since the last run. (e.g > 100 micro secs)
        ch_len >= READ_LOG_HIGH_WATER_MARK
    }

    fn should_apply_writes(&self, ch_len: usize) -> bool {
        // TODO: Also check how long past since the last run. (e.g > 100 micro secs)
        ch_len >= WRITE_LOG_HIGH_WATER_MARK
    }
}

impl<K, V, S> ConcurrentCache<K, V> for LFUCache<K, V, S>
where
    K: Clone + Debug + Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, key: &K) -> Option<Arc<V>> {
        let v = self.inner.get(key);
        self.record_read_op(key);
        v
    }

    fn get_or_insert(&self, _key: K, _default: V) -> Arc<V> {
        unimplemented!() // todo!() was introduced in Rust 1.40.0.
    }

    fn get_or_insert_with<F>(&self, _key: K, _default: F) -> Arc<V>
    where
        F: FnOnce() -> V,
    {
        unimplemented!()
    }

    fn insert(&self, key: K, value: V) {
        self.schedule_insert_op(key, value)
            .expect("Failed to insert");
    }

    fn remove(&self, key: &K) -> Option<Arc<V>> {
        self.schedule_remove_op(key).expect("Failed to remove");
        self.inner.get(key)
    }
}

unsafe impl<K, V, S> Send for LFUCache<K, V, S> {}
unsafe impl<K, V, S> Sync for LFUCache<K, V, S> {}

struct LFUInner<K, V, S> {
    capacity: usize,
    cache: Cache<K, V, S>,
    keys: Mutex<KeySet<K>>,
    frequency_sketch: RwLock<CountMinSketch8<K>>,
    reads_apply_lock: Mutex<()>,
    writes_apply_lock: Mutex<()>,
    read_op_ch: Receiver<K>,
    write_op_ch: Receiver<WriteOp<K, V>>,
}

impl<K, V, S> LFUInner<K, V, S>
where
    K: Clone + Debug + Eq + Hash,
    S: BuildHasher,
{
    fn new(
        capacity: usize,
        build_hasher: S,
        read_op_ch: Receiver<K>,
        write_op_ch: Receiver<WriteOp<K, V>>,
    ) -> Self {
        let skt_capacity = usize::max(capacity, 100);
        let frequency_sketch = CountMinSketch8::new(skt_capacity, 0.95, 10.0)
            .expect("Failed to create the frequency sketch");

        Self {
            capacity,
            cache: cht::HashMap::with_capacity_and_hasher(capacity, build_hasher),
            keys: Mutex::new(HashSet::default()),
            frequency_sketch: RwLock::new(frequency_sketch),
            reads_apply_lock: Mutex::new(()),
            writes_apply_lock: Mutex::new(()),
            read_op_ch,
            write_op_ch,
        }
    }

    fn get(&self, key: &K) -> Option<Arc<V>> {
        self.cache.get(key)
    }

    // fn get_or_insert_with<F>(&mut self, _key: K, _default: F) -> Arc<V>
    // where
    //     F: FnOnce() -> V,
    // {
    //     unimplemented!()
    // }

    // fn insert(&mut self, key: K, value: V) {
    //     println!(
    //         "insert() - estimated frequency of {:?}: {}",
    //         key,
    //         self.frequency_sketch.read().estimate(&key)
    //     );
    //     self.do_insert(key, Arc::new(value));
    // }

    fn apply_reads(&self, _lock: MutexGuard<'_, ()>, count: usize) {
        let mut freq = self.frequency_sketch.write();
        let ch = &self.read_op_ch;
        for _ in 0..count {
            match ch.try_recv() {
                Ok(key) => freq.increment(&key),
                Err(_) => break,
            }
        }
    }

    fn apply_writes(&self, _lock: MutexGuard<'_, ()>, count: usize) {
        let freq = self.frequency_sketch.read();
        let mut keys = self.keys.lock();

        let ch = &self.write_op_ch;
        for _ in 0..count {
            match ch.try_recv() {
                Ok(Insert(key, value)) => self.do_insert(key, Arc::new(value), &mut keys, &freq),
                Ok(Remove(key)) => {
                    keys.remove(&key);
                }
                Err(_) => break,
            };
        }
    }

    fn admit(&self, candidate: &K, victim: &K, freq: &CountMinSketch8<K>) -> bool {
        // TODO: Implement some randomness to mitigate hash DoS.
        freq.estimate(candidate) > freq.estimate(victim)
    }

    fn do_insert(&self, key: K, value: Arc<V>, keys: &mut KeySet<K>, freq: &CountMinSketch8<K>) {
        if self.cache.len() < self.capacity {
            let key = Arc::new(key);
            keys.insert(Arc::clone(&key));
            self.cache.insert(key, value);
        } else {
            let victim = self.find_cache_victim(keys, freq);
            if self.admit(&key, &victim, freq) {
                let key = Arc::new(key);
                {
                    keys.remove(&victim);
                    keys.insert(Arc::clone(&key));
                }
                self.cache.remove(&victim);
                self.cache.insert(key, value);
            }
        }
    }

    // TODO: Run this periodically in background.
    fn find_cache_victim(&self, keys: &mut KeySet<K>, freq: &CountMinSketch8<K>) -> Arc<K> {
        let mut victim = None;

        for key in keys.iter() {
            let freq0 = freq.estimate(key);
            match victim {
                None => victim = Some((freq0, key)),
                Some((freq1, _)) if freq0 < freq1 => victim = Some((freq0, key)),
                Some(_) => (),
            }
        }
        // TODO: Remove clone().
        // Maybe the cache map should have <Arc<K>, Arc<V>> instead of <K, Arc<V>>?
        let (_, k) = victim.expect("No victim found");
        Arc::clone(k)
    }
}

// To see the debug prints, run test as `cargo test -- --nocapture`
#[cfg(test)]
mod tests {
    use super::{ConcurrentCache, LFUCache};
    use std::hash::BuildHasherDefault;
    use std::sync::Arc;

    #[test]
    fn naive_basics() {
        let cache = LFUCache::new(3);
        cache.insert("a", "alice");
        cache.insert("b", "bob");
        cache.sync();

        assert_eq!(cache.get(&"a"), Some(Arc::new("alice")));
        assert_eq!(cache.get(&"b"), Some(Arc::new("bob")));
        assert_eq!(cache.get(&"a"), Some(Arc::new("alice")));
        assert_eq!(cache.get(&"b"), Some(Arc::new("bob")));
        // counts: a -> 2, b -> 2

        cache.insert("c", "cindy");
        cache.sync();

        assert_eq!(cache.get(&"c"), Some(Arc::new("cindy")));
        // counts: a -> 2, b -> 2, c -> 1

        // "d" should not be admitted because its frequency is too low.
        cache.insert("d", "david"); //        count: d -> 0
        cache.sync();
        assert_eq!(cache.get(&"d"), None); //        d -> 1

        cache.insert("d", "david");
        cache.sync();
        assert_eq!(cache.get(&"d"), None); //        d -> 2

        // "d" should be admitted and "c" should be evicted
        // because d's frequency is higher then c's.
        cache.insert("d", "dennis");
        cache.sync();
        assert_eq!(cache.get(&"d"), Some(Arc::new("dennis")));
        assert_eq!(cache.get(&"c"), None);

        assert_eq!(cache.remove(&"b"), Some(Arc::new("bob")));
    }
}
