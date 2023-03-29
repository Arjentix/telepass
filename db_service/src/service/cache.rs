//! Module with [`Cache`] structure used in [`Database Service`](super::Database) implementation.

use std::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    sync::RwLock,
};

use tracing::info;

use crate::models::Record;

mod last_seen;

/// Cache for [`Database Service`](super::Database).
///
/// Should be [pre-loaded](Self::load()) at construction time and correctly invalidated when data is changed.
#[derive(Debug)]
pub struct Cache {
    /// Records cache for [`get`](super::Database::get()) request.
    records: RwLock<last_seen::Set<String, ResourceOrientedRecord>>,
    /// Records cache for [`list`](super::Database::list()) request.
    resources: RwLock<Resources>,
}

/// Helper struct to store [`Record`]s and their validness.
#[derive(Debug)]
struct Resources {
    /// List of resources
    raw: Vec<String>,
    /// Validness flag
    valid: bool,
}

/// Helper struct that implements `Borrow<&str>`.
///
/// Behaves like [`Record::resource`] is the only field in the struct, which is useful
/// for [`HashSet`].
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
    pub fn load(size: u32, records: Vec<Record>) -> Self {
        let resources = RwLock::new(Resources {
            raw: records.iter().map(|r| r.resource.clone()).collect(),
            valid: true,
        });

        let size = size
            .try_into()
            .expect("`u32` should always fit into `usize`");
        let mut records_set = last_seen::Set::new(size);
        for record in records.into_iter().take(size).map(ResourceOrientedRecord) {
            records_set.insert(record);
        }

        Self {
            records: RwLock::new(records_set),
            resources,
        }
    }

    /// Add `record` to the cache.
    pub fn add(&self, record: Record) {
        {
            let mut resources_write = write_or_panic!(self.resources);
            if resources_write.valid {
                resources_write.raw.push(record.resource.clone());
            }
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
            resources_write.valid = false;
        }
    }

    /// Get record by resource name or insert it using `f`, if not presented.
    pub fn get_or_try_insert_with<F, E>(&self, resource_name: &String, f: F) -> Result<Record, E>
    where
        F: FnOnce() -> Result<Record, E>,
    {
        if let Some(record) = write_or_panic!(self.records).get(resource_name) {
            info!("Using cache");
            return Ok(record.0.clone());
        }

        let new_record = f()?;
        write_or_panic!(self.records).insert(ResourceOrientedRecord(new_record.clone()));

        let mut resources_write = write_or_panic!(self.resources);
        if resources_write.valid {
            resources_write.raw.push(new_record.resource.clone());
        }

        Ok(new_record)
    }

    /// Get all records or insert them using `f`, if not presented.
    pub fn get_all_or_try_insert_with<F, E>(&self, f: F) -> Result<Vec<String>, E>
    where
        F: FnOnce() -> Result<Vec<String>, E>,
    {
        {
            let resources_read = read_or_panic!(self.resources);
            if resources_read.valid {
                info!("Using cache");
                return Ok(resources_read.raw.clone());
            }
        }

        let new_resources = f()?;
        let mut resources_write = write_or_panic!(self.resources);
        resources_write.raw = new_resources.clone();
        resources_write.valid = true;

        Ok(new_resources)
    }
}
