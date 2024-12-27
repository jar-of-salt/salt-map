// this is still memory inefficient, since each element is a Vec
#[derive(Debug)]
pub struct ChainingHashMap<K: Eq + std::hash::Hash, V, S = std::hash::RandomState> {
    backing: Vec<Option<Vec<(K, V)>>>,
    load: usize,
    load_factor: f32, // reduce the result to the scale expected by a bucket
    hash_builder: S,
}

fn make_backing_with_capacity<K, V>(capacity: usize) -> Vec<Option<Vec<(K, V)>>> {
    let mut backing_vec = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        backing_vec.push(None);
    }
    backing_vec
}

impl<K: Eq + std::hash::Hash, V> ChainingHashMap<K, V, std::hash::RandomState> {
    pub fn with_capacity(capacity: usize) -> Self {
        ChainingHashMap {
            backing: make_backing_with_capacity::<K, V>(capacity),
            load: 0,
            load_factor: 0.7,
            hash_builder: std::hash::RandomState::new(),
        }
    }

    pub fn new() -> ChainingHashMap<K, V> {
        // TODO: figure out if this is a good starting capacity, or if we can go lower
        ChainingHashMap::with_capacity(20)
    }
}

impl<K: Eq + std::hash::Hash, V, S> ChainingHashMap<K, V, S> {
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> ChainingHashMap<K, V, S> {
        ChainingHashMap {
            backing: make_backing_with_capacity::<K, V>(capacity),
            load: 0,
            load_factor: 0.7,
            hash_builder: hash_builder,
        }
    }

    pub fn with_hasher(hash_builder: S) -> ChainingHashMap<K, V, S> {
        ChainingHashMap::with_capacity_and_hasher(20, hash_builder)
    }
}

impl<K: Eq + std::hash::Hash, V> ChainingHashMap<K, V> {
    fn get_index(&self, key: &K) -> usize {
        use std::hash::BuildHasher;
        let mut hasher = self.hash_builder.build_hasher();

        key.hash(&mut hasher);

        use std::hash::Hasher;
        hasher.finish() as usize % self.backing.capacity()
    }

    // consider impl of new
    pub fn len(&self) -> usize {
        self.load
    }

    pub fn capacity(&self) -> usize {
        self.backing.capacity()
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // resize before getting index, otherwise it will be the index for the previous capacity
        if self.len() as f32 / self.capacity() as f32 > self.load_factor {
            self.resize();
        }

        let idx = self.get_index(&key);

        let entry = std::mem::replace(&mut self.backing[idx], None);

        // TODO: implement a resizing protocol once len / capacity > load factor

        match entry {
            None => {
                self.backing[idx] = Some(vec![(key, value)]);
                self.load += 1;
                None
            }
            Some(mut vec) => {
                let mut result = None;
                for item in vec.iter_mut() {
                    if key == item.0 {
                        result = Some(std::mem::replace(&mut item.1, value));
                        self.backing[idx] = Some(vec);
                        return result;
                    }
                }

                vec.push((key, value));
                self.load += 1;

                self.backing[idx] = Some(vec);

                result
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let idx = self.get_index(&key);

        self.backing
            .get(idx)?
            .as_ref()?
            .iter()
            .filter(|item| *key == item.0)
            .collect::<Vec<&(K, V)>>()
            .first()
            .map(|item| &item.1)
    }

    fn resize(&mut self) -> () {
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
        let old_backing = std::mem::replace(&mut self.backing, new_backing);

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
            self.backing[idx]
                .as_mut()
                .map(|vec| vec.remove(*internal_idx))
        })
    }

    // Removes the value related to the given key, returning an Option containing its value if it
    // is present
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.remove_entry(key).map(|entry| entry.1)
    }
}

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
}
