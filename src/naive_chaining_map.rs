fn hash(key: &str, salt: &Option<&usize>) -> usize {
    let bytes: &[u8] = key.as_bytes();
    let mut result: usize = 0;

    // apply a few bitshifts
    if let Some(value) = salt {
        // salt for increased randomness
        result += *value;
    }
    // rudimentary hashing algorithm, combining pairs of bytes, bit shifting them left
    // by a power of the shift_length
    for chunk in bytes.chunks(2) {
        for (idx, byte) in chunk.iter().enumerate() {
            result += (*byte as usize) << 2_usize.pow(idx.try_into().unwrap());
        }
    }

    result
}

// this is still memory inefficient, since each element is a Vec
#[derive(Debug)]
pub struct ChainingHashMap<V> {
    backing: Vec<Option<Vec<(String, V)>>>,
    salt: Option<usize>,
    load: usize,
    load_factor: f32, // reduce the result to the scale expected by a bucket
}

impl<V> ChainingHashMap<V> {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut backing_vec = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            backing_vec.push(None);
        }
        ChainingHashMap {
            backing: backing_vec,
            salt: None,
            load: 0,
            load_factor: 0.7,
        }
    }

    pub fn new() -> ChainingHashMap<V> {
        // TODO: figure out if this is a good starting capacity, or if we can go lower
        ChainingHashMap::with_capacity(20)
    }
}

impl<V> ChainingHashMap<V> {
    fn get_index(&self, key: &String) -> usize {
        let idx = hash(key, &self.salt.as_ref());

        idx % self.backing.capacity()
    }

    // consider impl of new
    pub fn len(&self) -> usize {
        self.load
    }

    pub fn capacity(&self) -> usize {
        self.backing.capacity()
    }

    pub fn insert(&mut self, key: String, value: V) -> Option<V> {
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

    pub fn get(&self, key: &String) -> Option<&V> {
        let idx = self.get_index(&key);

        match &self.backing.get(idx) {
            None => None,
            Some(opt_vec) => match opt_vec {
                None => None,
                Some(vec) => {
                    for item in vec {
                        if *key == item.0 {
                            return Some(&item.1);
                        }
                    }
                    None
                }
            },
        }
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
        assert_eq!(result3, Some(123));

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
}
