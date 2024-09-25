//! Module with [`Set`] that stores only specified number of records, replacing the least popular
//! one on insertion.

#![allow(
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "used to indicate programmer errors"
)]

use std::{borrow::Borrow, collections::HashSet, hash::Hash, marker::PhantomData};

/// [`Set`] that stores only specified number of records, replacing the least popular one on
/// insertion.
///
/// # Generics
///
/// `V` - value type to be stored in [`Set`].
/// `Q` - value that can be borrowed from `V` and used in lookup functions like [`get()`](Set::get).
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
pub struct Set<V, Q: ?Sized = V> {
    /// Internal set with values and rates.
    internal: HashSet<ValueWithRate<V, Q>>,
    /// Maximum number of values to store.
    capacity: usize,
}

/// Value wrapper, which also stores rate of usage.
#[derive(Debug)]
struct ValueWithRate<V, Q: ?Sized> {
    /// Value.
    value: V,
    /// Usage rate. Increased by one every time value is accessed.
    rate: u128,
    /// Phantom data to implement [`Deref`](std::ops::Deref) without conflicts.
    _phantom_key: PhantomData<Q>,
}

impl<V: Clone, Q: ?Sized> Clone for ValueWithRate<V, Q> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            rate: self.rate,
            _phantom_key: PhantomData,
        }
    }
}

impl<V: PartialEq, Q: ?Sized> PartialEq for ValueWithRate<V, Q> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<V: Eq, Q: ?Sized> Eq for ValueWithRate<V, Q> {}

impl<V: Hash, Q: ?Sized + Hash> Hash for ValueWithRate<V, Q> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<V: Borrow<Q>, Q: ?Sized> Borrow<Q> for ValueWithRate<V, Q> {
    fn borrow(&self) -> &Q {
        self.value.borrow()
    }
}

impl<V: Borrow<Q> + Eq + Hash, Q: Clone + Eq + Hash> Set<V, Q> {
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
    /// O(n) in general but O(1) while size < capacity
    pub fn insert(&mut self, value: V) -> Option<V> {
        if self.internal.len() == self.capacity {
            let key = self
                .internal
                .iter()
                .min_by_key(|value_with_rate| value_with_rate.rate)
                .map(|value_with_rate| -> Q { value_with_rate.value.borrow().clone() })
                .expect("`Set::internal` guaranteed to be non-empty");

            self.internal.remove(&key);
        }

        self.internal
            .replace(ValueWithRate {
                value,
                rate: 1,
                _phantom_key: PhantomData,
            })
            .map(|value_with_rate| value_with_rate.value)
    }

    /// Remove value from the set. Returns removed value if it was present.
    ///
    /// # Complexity
    ///
    /// O(1)
    pub fn remove(&mut self, value: &Q) -> Option<V> {
        self.internal
            .take(value)
            .map(|value_with_rate| value_with_rate.value)
    }

    /// Get value from the set if it's present.
    ///
    /// Updates rate of the value.
    ///
    /// # Complexity
    ///
    /// O(1)
    pub fn get(&mut self, value: &Q) -> Option<&V> {
        let mut value_with_rate = self.internal.take(value)?;

        // Probably will never overflow. If overflow happens, then don't change value
        value_with_rate.rate = value_with_rate
            .rate
            .checked_add(1)
            .unwrap_or(value_with_rate.rate);
        assert!(
            self.internal.insert(value_with_rate),
            "Value guaranteed to not be present in `Set::internal` at this moment"
        );

        self.internal.get(value).map(|with_rate| &with_rate.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get_should_work() {
        let mut set = Set::<u32>::new(3);
        set.insert(0);
        set.insert(1);
        set.insert(2);

        assert!(matches!(set.get(&1), Some(&1)));
        assert!(set.get(&3).is_none());
    }

    #[test]
    fn remove_should_work() {
        let mut set = Set::<u32>::new(3);
        set.insert(0);
        set.insert(1);
        set.insert(2);

        set.remove(&1);
        assert!(set.get(&1).is_none());
    }

    #[test]
    fn insert_should_not_replace_until_set_is_full() {
        let mut set = Set::<u32>::new(3);
        set.insert(0);
        assert!(matches!(set.get(&0), Some(&0)));
        set.insert(1);
        assert!(matches!(set.get(&1), Some(&1)));
        set.insert(2);
        assert!(matches!(set.get(&2), Some(&2)));
    }

    #[test]
    fn insert_should_replace_the_least_usable() {
        let mut set = Set::<u32>::new(3);
        set.insert(0);
        set.insert(1);
        set.insert(2);

        // Increasing usage rates
        set.get(&0);
        set.get(&1);
        set.get(&1);
        set.get(&2);
        set.get(&2);

        set.insert(3);
        assert!(matches!(set.get(&3), Some(&3)));
        assert!(set.get(&0).is_none());
    }
}
