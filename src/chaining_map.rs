use std::hash;
use std::mem;

// this is still memory inefficient, since each element is a Vec
#[derive(Debug)]
pub struct ChainingHashMap<K, V, S = hash::RandomState> {
    backing: Vec<Option<Vec<(K, V)>>>,
    load: usize,
    load_factor: f32, // reduce the result to the scale expected by a bucket
    hash_builder: S,
}

fn make_backing_with_capacity<K, V>(capacity: usize, load_factor: f32) -> Vec<Option<Vec<(K, V)>>> {
    // makes a backing with an effective capacity of the given capacity, actual capacity of
    // capacity / load factor; this ensures the map can hold at least `capacity` before
    // reallocating
    let modified_capacity = (capacity as f32 / load_factor) as usize;
    let mut backing_vec = Vec::with_capacity(modified_capacity);
    for _ in 0..modified_capacity {
        backing_vec.push(None);
    }
    backing_vec
}

impl<K, V> ChainingHashMap<K, V, hash::RandomState> {
    pub fn with_capacity(capacity: usize) -> Self {
        let load_factor = 0.7;
        ChainingHashMap {
            backing: make_backing_with_capacity::<K, V>(capacity, load_factor),
            load: 0,
            load_factor: load_factor,
            hash_builder: hash::RandomState::new(),
        }
    }

    pub fn new() -> Self {
        // TODO: figure out if this is a good starting capacity, or if we can go lower
        ChainingHashMap::with_capacity(20)
    }
}

impl<K, V, S> ChainingHashMap<K, V, S> {
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        let load_factor = 0.7;
        ChainingHashMap {
            backing: make_backing_with_capacity::<K, V>(capacity, load_factor),
            load: 0,
            load_factor: load_factor,
            hash_builder: hash_builder,
        }
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        ChainingHashMap::with_capacity_and_hasher(20, hash_builder)
    }

    pub fn capacity(&self) -> usize {
        // TODO: go over the semantics of capacity to make sure they make sense; i.e. need to make
        // sure the rules for when reallocation happens make sense
        self.backing.capacity()
    }

    pub fn len(&self) -> usize {
        self.load
    }

    pub fn is_empty(&self) -> bool {
        self.load == 0
    }

    pub fn clear(&mut self) {
        self.load = 0;
        self.backing.iter_mut().for_each(|x| *x = None)
    }

    pub fn hasher(&self) -> &S {
        &self.hash_builder
    }
}

impl<K, V, S> ChainingHashMap<K, V, S>
where
    K: Eq + hash::Hash,
    S: hash::BuildHasher,
{
    fn get_index(&self, key: &K) -> usize {
        // builds a hash with the instance's `hash_builder`, using the `BuildHasher` trait
        let mut hasher = self.hash_builder.build_hasher();

        key.hash(&mut hasher);

        use hash::Hasher;
        hasher.finish() as usize % self.backing.capacity()
    }

    // TODO: try to make this more idiomatic
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // resize before getting index, otherwise it will be the index for the previous capacity
        // TODO: possibly make internal insert to make the reserve/shrink functions work on
        // reallocation
        if self.len() as f32 / self.capacity() as f32 > self.load_factor {
            self.resize();
        }

        let idx = self.get_index(&key);

        match mem::replace(&mut self.backing[idx], None) {
            None => {
                self.backing[idx] = Some(vec![(key, value)]);
                self.load += 1;
                None
            }
            Some(mut vec) => {
                for item in vec.iter_mut() {
                    if key == item.0 {
                        let result = Some(mem::replace(&mut item.1, value));
                        self.backing[idx] = Some(vec);
                        return result;
                    }
                }

                vec.push((key, value));
                self.load += 1;

                self.backing[idx] = Some(vec);

                None
            }
        }
    }

    /// Gets reference to value based on the input key
    pub fn get(&self, key: &K) -> Option<&V> {
        self.backing
            .get(self.get_index(&key))?
            .as_ref()?
            .iter()
            .find(|item| *key == item.0)
            .map(|item| &item.1)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = self.get_index(&key);
        self.backing
            .get_mut(idx)?
            .as_mut()?
            .iter_mut()
            .find(|item| *key == item.0)
            .map(|item| &mut item.1)
    }

    fn resize(&mut self) {
        // resizes by exponentially doubling the capacity

        // double the capacity
        let new_cap = self.capacity() * 2;

        // fill the new backing
        let mut new_backing = Vec::with_capacity(new_cap);
        for _ in 0..new_cap {
            new_backing.push(None);
        }

        // reset the load
        self.load = 0;

        // replace the old backing and extract it
        let old_backing = mem::replace(&mut self.backing, new_backing);

        for item in old_backing.into_iter() {
            // for each item in the old backing, check if it has a vec inside, iterate over the vec
            if let Some(vec) = item {
                for entry in vec {
                    self.insert(entry.0, entry.1);
                }
            }
        }
    }

    pub fn remove_entry(&mut self, key: &K) -> Option<(K, V)> {
        let idx = self.get_index(key);

        let indices_vec = self
            .backing
            .get(idx)?
            .as_ref()?
            .iter()
            .enumerate()
            .filter(|item: &(usize, &(K, V))| *key == item.1 .0)
            .map(|item: (usize, &(K, V))| item.0)
            .collect::<Vec<usize>>();

        indices_vec.first().and_then(|internal_idx| {
            let item = self.backing[idx]
                .as_mut()
                .map(|vec| vec.remove(*internal_idx));
            if item.is_some() {
                self.load -= 1;
            }

            item
        })
    }

    /// Removes the value related to the given key, returning an Option containing its value if it
    /// is present
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.remove_entry(key).map(|entry| entry.1)
    }
}

// TODO: implement benchmarks for insert/get
// See: https://doc.rust-lang.org/unstable-book/library-features/test.html
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut map = ChainingHashMap::new();

        let result1 = map.insert("yes".to_string(), 123);
        assert_eq!(result1, None);

        let result2 = map.insert("no".to_string(), 456);
        assert_eq!(result2, None);

        let result3 = map.insert("yes".to_string(), 456);
        assert_eq!(result3.unwrap(), 123);

        assert_eq!(map.len(), 2)
    }

    #[test]
    fn insert_and_resize() {
        let cap = 10;
        let mut map = ChainingHashMap::with_capacity(cap);

        for i in 0..cap {
            let result = map.insert(i.to_string(), i);
            assert_eq!(result, None);
        }

        for i in 0..cap {
            let result = map.get(&i.to_string());
            assert_eq!(result, Some(i).as_ref());
        }
    }

    #[test]
    fn get() {
        let mut map = ChainingHashMap::new();

        map.insert("yes".to_string(), 123);
        map.insert("no".to_string(), 456);

        assert_eq!(map.get(&"yes".to_string()), Some(123).as_ref());
        assert_eq!(map.get(&"no".to_string()), Some(456).as_ref());
        assert_eq!(map.get(&"maybe".to_string()), None.as_ref());
    }

    #[test]
    fn get_mut() {
        let mut map = ChainingHashMap::new();

        map.insert("yes".to_string(), 123);

        assert_eq!(map.get(&"yes".to_string()), Some(123).as_ref());

        let item = map.get_mut(&"yes".to_string());

        assert!(item.is_some(), "Expected entry is not present");

        if let Some(value) = item {
            *value += 1;
        }

        assert_eq!(map.get(&"yes".to_string()), Some(124).as_ref());
    }

    #[test]
    fn remove_entry() {
        let cap = 10;
        let mut map = ChainingHashMap::with_capacity(cap);

        for i in 0..cap {
            let result = map.insert(i.to_string(), i);
            assert_eq!(result, None);
        }

        assert_eq!(map.remove(&"100".to_string()), None);

        for i in 0..cap {
            let result = map.remove_entry(&i.to_string());
            assert_eq!(result.unwrap(), (i.to_string(), i));
        }
    }

    #[test]
    fn remove() {
        let cap = 10;
        let mut map = ChainingHashMap::with_capacity(cap);

        for i in 0..cap {
            let result = map.insert(i.to_string(), i);
            assert_eq!(result, None);
        }

        assert_eq!(map.remove(&"100".to_string()), None);

        for i in 0..cap {
            let result = map.remove(&i.to_string());
            assert_eq!(result.unwrap(), i);
        }
    }

    #[test]
    fn grow_and_shrink_len() {
        let cap = 10;
        let mut map = ChainingHashMap::with_capacity(cap);

        for i in 0..cap {
            let result = map.insert(i.to_string(), i);
            assert_eq!(result, None);
            assert_eq!(map.len(), i + 1);
        }

        assert_eq!(map.remove(&"100".to_string()), None);

        for i in 0..cap {
            let result = map.remove(&i.to_string());
            assert_eq!(result.unwrap(), i);
            assert_eq!(map.len(), 10 - i - 1);
        }

        for i in 0..cap {
            let result = map.insert(i.to_string(), i);
            assert_eq!(result, None);
            assert_eq!(map.len(), i + 1);
        }

        assert_eq!(map.remove(&"100".to_string()), None);

        for i in 0..cap {
            let result = map.remove_entry(&i.to_string());
            assert_eq!(result.unwrap(), (i.to_string(), i));
            assert_eq!(map.len(), 10 - i - 1);
        }
    }

    #[test]
    fn clear() {
        let cap = 100;
        let mut map = ChainingHashMap::with_capacity(cap);

        for i in 0..cap {
            map.insert(i.to_string(), i);
        }

        assert_eq!(map.len(), cap);

        map.clear();

        assert_eq!(map.len(), 0);
    }
}
