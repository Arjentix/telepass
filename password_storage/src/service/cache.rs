//! Module with [`Cache`] structure used in [`PasswordStorage Service`](super::PasswordStorage) implementation.

use std::{
    borrow::Borrow,
    collections::BTreeSet,
    hash::{Hash, Hasher},
    sync::RwLock,
};

use tracing::info;

use crate::models::Record;

mod rated;

/// Cache for [`PasswordStorage Service`](super::PasswordStorage).
///
/// Should be [pre-loaded](Self::load()) at construction time and correctly invalidated when data is changed.
#[derive(Debug)]
pub struct Cache {
    /// Records cache for [`get`](crate::grpc::password_storage_server::PasswordStorage::get) request.
    records: RwLock<rated::Set<ResourceOrientedRecord, String>>,
    /// Cache of sorted resources for [`list`](crate::grpc::password_storage_server::PasswordStorage::list) request.
    /// Always in actual state.
    resources: RwLock<BTreeSet<String>>,
}

/// Helper struct that implements `Borrow<&str>`.
///
/// Behaves like [`Record::resource`] is the only field in the struct, which is useful
/// for [`rated::Set`].
#[derive(Debug)]
struct ResourceOrientedRecord(Record);

impl PartialEq for ResourceOrientedRecord {
    fn eq(&self, other: &Self) -> bool {
        self.0.resource == other.0.resource
    }
}

impl Eq for ResourceOrientedRecord {}

impl Hash for ResourceOrientedRecord {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.resource.hash(state);
    }
}

impl Borrow<String> for ResourceOrientedRecord {
    fn borrow(&self) -> &String {
        &self.0.resource
    }
}

/// Macro to remove boilerplate code for reading from [`RwLock`].
///
/// Calls [`RwLock::read()`](std::sync::RwLock::read())
/// and panics if it returns [`PoisonError`](std::sync::PoisonError).
macro_rules! read_or_panic {
    ($lock:expr) => {
        $lock.read().expect(concat!(
            "`",
            stringify!($lock),
            "` should not be poisoned while trying to read"
        ))
    };
}

/// Macro to remove boilerplate code for writing to [`RwLock`].
///
/// Calls [`RwLock::write()`](std::sync::RwLock::read())
/// and panics if it returns [`PoisonError`](std::sync::PoisonError).
macro_rules! write_or_panic {
    ($lock:expr) => {
        $lock.write().expect(concat!(
            "`",
            stringify!($lock),
            "` should not be poisoned while trying to write"
        ))
    };
}

impl Cache {
    /// Creates new [`Cache`] instance with max size `size` and pre-loaded `records`.
    ///
    /// All records after `size - 1` index will be ignored.
    #[allow(clippy::expect_used)]
    pub fn load(size: u32, records: impl IntoIterator<Item = Record>) -> Self {
        let size = size
            .try_into()
            .expect("`u32` should always fit into `usize`");

        let mut resources = BTreeSet::new();
        let mut records_set = rated::Set::new(size);
        for (n, record) in records.into_iter().enumerate() {
            resources.insert(record.resource.clone());

            if n < size {
                records_set.insert(ResourceOrientedRecord(record));
            }
        }

        Self {
            records: RwLock::new(records_set),
            resources: RwLock::new(resources),
        }
    }

    /// Add `record` to the cache.
    pub fn add(&self, record: Record) {
        {
            let mut resources_write = write_or_panic!(self.resources);
            resources_write.insert(record.resource.clone());
        }
        {
            let mut records_write = write_or_panic!(self.records);
            records_write.insert(ResourceOrientedRecord(record));
        }
    }

    /// Invalidate record by resource name.
    pub fn invalidate(&self, resource_name: &String) {
        {
            let mut records_write = write_or_panic!(self.records);
            records_write.remove(resource_name);
        }
        {
            let mut resources_write = write_or_panic!(self.resources);
            resources_write.remove(resource_name);
        }
    }

    /// Get record by resource name or insert it using `f`, if not presented.
    pub fn get_or_try_insert_with<F, E>(&self, resource_name: &String, f: F) -> Result<Record, E>
    where
        F: FnOnce() -> Result<Record, E>,
    {
        let new_record = {
            let mut records_write = write_or_panic!(self.records);
            if let Some(record) = records_write.get(resource_name) {
                info!("Using cache");
                return Ok(record.0.clone());
            }

            let new_record = f()?;
            records_write.insert(ResourceOrientedRecord(new_record.clone()));
            new_record
        };

        write_or_panic!(self.resources).insert(new_record.resource.clone());

        Ok(new_record)
    }

    /// Get all resources
    pub fn get_all_resources(&self) -> BTreeSet<String> {
        info!("Using cache");
        read_or_panic!(self.resources).clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::default_numeric_fallback)]
mod tests {
    use std::convert::Infallible;

    use super::*;

    #[test]
    fn load_should_take_exact_size_records() {
        let cache = Cache::load(3, create_records(5));

        let presented_record = cache
            .get_or_try_insert_with(
                &String::from("Sample resource #2"),
                || -> Result<_, Infallible> { panic!("Shouldn't be called") },
            )
            .unwrap();
        assert_eq!(
            presented_record,
            Record {
                resource: String::from("Sample resource #2"),
                passhash: String::from("some_secret_hash_2"),
                salt: String::from("some_salt_2"),
            }
        );

        let sample_record = Record {
            resource: String::from("Sample sample"),
            passhash: String::from("sample"),
            salt: String::from("sample"),
        };
        let not_presented_record = cache
            .get_or_try_insert_with(
                &String::from("Sample resource #3"),
                || -> Result<_, Infallible> { Ok(sample_record.clone()) },
            )
            .unwrap();
        assert_eq!(not_presented_record, sample_record);
    }

    #[test]
    fn load_should_take_all_resource_names() {
        let cache = Cache::load(3, create_records(10));

        let resources = cache.get_all_resources();
        assert_eq!(
            resources,
            create_records(10)
                .into_iter()
                .map(|record| record.resource)
                .collect()
        );
    }

    #[test]
    fn add_should_work() {
        let cache = Cache::load(3, create_records(2));

        let resource = String::from("Sample sample");
        let sample_record = Record {
            resource: resource.clone(),
            passhash: String::from("sample"),
            salt: String::from("sample"),
        };
        cache.add(sample_record.clone());

        let record = cache
            .get_or_try_insert_with(&resource, || -> Result<_, Infallible> {
                panic!("Shouldn't be called")
            })
            .unwrap();
        assert_eq!(record, sample_record);

        let resources = cache.get_all_resources();
        assert_eq!(
            resources,
            create_records(2)
                .into_iter()
                .chain(std::iter::once(sample_record))
                .map(|r| r.resource)
                .collect()
        );
    }

    #[test]
    fn add_should_replace_the_least_usable() {
        let cache = Cache::load(3, create_records(3));

        // Increasing usage rate
        for i in (0..3).chain(1..3) {
            cache
                .get_or_try_insert_with(
                    &format!("Sample resource #{i}"),
                    || -> Result<_, Infallible> { panic!("Shouldn't be called") },
                )
                .unwrap();
        }

        let sample_record = Record {
            resource: String::from("Sample sample"),
            passhash: String::from("sample"),
            salt: String::from("sample"),
        };
        cache.add(sample_record);

        let mut called = false;
        cache
            .get_or_try_insert_with(
                &String::from("Sample resource #0"),
                || -> Result<_, Infallible> {
                    called = true;
                    Ok(create_records(1).into_iter().next().unwrap())
                },
            )
            .unwrap();
        assert!(called);
    }

    #[test]
    fn invalidate_should_work() {
        let cache = Cache::load(3, create_records(3));

        let resource = String::from("Sample resource #1");
        cache.invalidate(&resource);

        let new_sample_record = Record {
            resource: resource.clone(),
            passhash: String::from("new sample"),
            salt: String::from("new sample"),
        };
        let new_record = cache
            .get_or_try_insert_with(&resource, || -> Result<_, Infallible> {
                Ok(new_sample_record.clone())
            })
            .unwrap();
        assert_eq!(new_record, new_sample_record);
    }

    fn create_records(n: usize) -> impl IntoIterator<Item = Record> {
        (0..n).map(|i| Record {
            resource: format!("Sample resource #{i}"),
            passhash: format!("some_secret_hash_{i}"),
            salt: format!("some_salt_{i}"),
        })
    }
}
