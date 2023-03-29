//! Module with [`Set`] to store only specified number of last seen records.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use std::{borrow::Borrow, collections::HashSet, hash::Hash, marker::PhantomData};

/// [`Set`] to store only specified number of last seen records.
///
/// # Generics
///
/// `K` - key type, which is used to identify values and could be borrowed from `V`.
/// `V` - value type.
///
/// # Complexity
///
/// Provides *O(1)* complexity for [`remove()`](Self::remove) and [`get()`](Self::get) operations,
/// but *O(n)* for [`insert()`](Self::insert).
/// However it will be *O(1)* for [`insert()`](Self::insert) while *size < capacity*.
///
/// In other words it's not optimized for frequent updates but rather for frequent reads, which
/// is okay for our case, because we can adjust the capacity.
///
/// The best way would be the capacity to be bigger than the possible number of records in the
/// database. Considering we have only 1 user (which is the case for this MVP), it's not a problem.
#[derive(Debug)]
pub struct Set<K: ?Sized, V> {
    /// Internal set with values and rates.
    internal: HashSet<ValueWithRate<K, V>>,
    /// Maximum number of values to store.
    capacity: usize,
}

/// Value wrapper, which also stores rate of usage.
#[derive(Debug)]
struct ValueWithRate<K: ?Sized, V> {
    /// Value.
    value: V,
    /// Usage rate. Increased by one every time value is accessed.
    rate: u128,
    /// Phantom data to implement [`Deref`] without conflicts.
    _phantom_key: PhantomData<K>,
}

impl<K: ?Sized, V: Clone> Clone for ValueWithRate<K, V> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            rate: self.rate,
            _phantom_key: PhantomData::default(),
        }
    }
}

impl<K: ?Sized, V: PartialEq> PartialEq for ValueWithRate<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<K: ?Sized, V: Eq> Eq for ValueWithRate<K, V> {}

impl<K: ?Sized, V: Hash> Hash for ValueWithRate<K, V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<K: ?Sized, V: Borrow<K>> Borrow<K> for ValueWithRate<K, V> {
    fn borrow(&self) -> &K {
        self.value.borrow()
    }
}

impl<K: ?Sized + Clone + Eq + Hash, V: Borrow<K> + Eq + Hash> Set<K, V> {
    /// Create new instance of [`Set`].
    ///
    /// For more details about `capacity` see [`Set`].
    pub fn new(capacity: usize) -> Self {
        Self {
            internal: HashSet::with_capacity(capacity),
            capacity,
        }
    }

    /// Insert `value` into the set. Returns previous value if it was present.
    ///
    /// # Complexity
    ///
    /// O(n)
    pub fn insert(&mut self, value: V) -> Option<V> {
        if self.internal.len() == self.capacity {
            let key = self
                .internal
                .iter()
                .min_by_key(|value_with_rate| value_with_rate.rate)
                .map(|value_with_rate| -> K { value_with_rate.value.borrow().clone() })
                .expect("`Set::internal` guaranteed to be non-empty");

            self.internal.remove(&key);
        }

        self.internal
            .replace(ValueWithRate {
                value,
                rate: 1,
                _phantom_key: PhantomData::default(),
            })
            .map(|value_with_rate| value_with_rate.value)
    }

    /// Remove value with `key` from the set. Returns removed value if it was present.
    ///
    /// # Complexity
    ///
    /// O(1)
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.internal
            .take(key)
            .map(|value_with_rate| value_with_rate.value)
    }

    /// Get value with `key` from the set if it's present.
    ///
    /// Updates rate of the value.
    ///
    /// # Complexity
    ///
    /// O(1)
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let Some(mut value_with_rate) = self.internal.take(key) else {
            return None;
        };

        // Probably will never overflow. If overflow happens, then don't change value
        value_with_rate.rate = value_with_rate
            .rate
            .checked_add(1)
            .unwrap_or(value_with_rate.rate);
        assert!(
            self.internal.insert(value_with_rate),
            "Value guaranteed to not be present in `Set::internal` at this moment"
        );

        self.internal.get(key).map(|with_rate| &with_rate.value)
    }
}
