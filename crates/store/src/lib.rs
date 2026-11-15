//! One embedded store per server, one [`Keyspace`] per module.
//!
//! A module never sees the store, only its keyspace, and a keyspace can
//! address nothing outside its own prefix. Data another module owns is
//! reached through that module, not through its keys.

use std::path::Path;
use std::sync::Arc;

use kevy_embedded::{AppendFsync, Config};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tendhost_kernel::{Error, Result};

#[derive(Clone)]
pub struct Store {
    inner: kevy_embedded::Store,
    // keeps a temporary store's directory alive as long as any handle
    _temp: Option<Arc<tempfile::TempDir>>,
}

impl Store {
    /// Opens the store under `dir`, creating it when absent. Every write is
    /// on disk before the call returns.
    pub fn open(dir: &Path) -> Result<Self> {
        let config = Config::default()
            .with_persist(dir)
            .with_appendfsync(AppendFsync::Always);
        let inner = kevy_embedded::Store::open(config).map_err(Error::internal)?;
        Ok(Self { inner, _temp: None })
    }

    /// A real store in a directory that is removed with the last handle.
    pub fn temp() -> Result<Self> {
        let dir = tempfile::tempdir().map_err(Error::internal)?;
        let mut store = Self::open(dir.path())?;
        store._temp = Some(Arc::new(dir));
        Ok(store)
    }

    /// The keyspace of the module called `module`.
    pub fn keyspace(&self, module: &str) -> Keyspace {
        assert!(
            !module.is_empty() && module.bytes().all(|b| b.is_ascii_lowercase()),
            "a keyspace is named after a module: lowercase letters only"
        );
        Keyspace {
            store: self.inner.clone(),
            prefix: format!("{module}:"),
            _temp: self._temp.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Keyspace {
    store: kevy_embedded::Store,
    prefix: String,
    _temp: Option<Arc<tempfile::TempDir>>,
}

impl Keyspace {
    fn key(&self, key: &str) -> Vec<u8> {
        format!("{}{key}", self.prefix).into_bytes()
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.store.get(&self.key(key)).map_err(Error::internal)? {
            Some(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).map_err(Error::internal)?,
            )),
            None => Ok(None),
        }
    }

    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(Error::internal)?;
        self.store
            .set(&self.key(key), &bytes)
            .map_err(Error::internal)?;
        Ok(())
    }

    /// Writes only when the key is absent; `false` means it was taken.
    pub fn put_new<T: Serialize>(&self, key: &str, value: &T) -> Result<bool> {
        let bytes = serde_json::to_vec(value).map_err(Error::internal)?;
        self.store
            .setnx(&self.key(key), &bytes)
            .map_err(Error::internal)
    }

    /// `true` when the key existed.
    pub fn delete(&self, key: &str) -> Result<bool> {
        Ok(self
            .store
            .del(&[self.key(key).as_slice()])
            .map_err(Error::internal)?
            > 0)
    }

    /// Adds `member` to the ordered index `index` at `score`, moving it when
    /// already present.
    pub fn index_put(&self, index: &str, score: i64, member: &str) -> Result<()> {
        self.store
            .zadd(&self.key(index), &[(score as f64, member.as_bytes())])
            .map_err(Error::internal)?;
        Ok(())
    }

    pub fn index_remove(&self, index: &str, member: &str) -> Result<()> {
        self.store
            .zrem(&self.key(index), &[member.as_bytes()])
            .map_err(Error::internal)?;
        Ok(())
    }

    /// Members with `min <= score <= max`, ascending.
    pub fn index_range(&self, index: &str, min: i64, max: i64) -> Result<Vec<String>> {
        let raw = self
            .store
            .zrange_by_score(&self.key(index), min as f64, max as f64)
            .map_err(Error::internal)?;
        raw.into_iter()
            .map(|(member, _)| String::from_utf8(member).map_err(Error::internal))
            .collect()
    }

    /// Every member, ascending by score.
    pub fn index_all(&self, index: &str) -> Result<Vec<String>> {
        self.index_range(index, i64::MIN, i64::MAX)
    }

    /// Removes every member with `score <= max`; returns how many.
    pub fn index_trim(&self, index: &str, max: i64) -> Result<usize> {
        self.store
            .zremrangebyscore(&self.key(index), f64::MIN, max as f64)
            .map_err(Error::internal)
    }

    /// The highest score in the index; `None` when it is empty.
    pub fn index_max(&self, index: &str) -> Result<Option<i64>> {
        let last = self
            .store
            .zrange(&self.key(index), -1, -1)
            .map_err(Error::internal)?;
        Ok(last.first().map(|(_, score)| *score as i64))
    }

    pub fn index_len(&self, index: &str) -> Result<usize> {
        self.store
            .zcount(&self.key(index), f64::MIN, f64::MAX)
            .map_err(Error::internal)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Doc {
        n: u32,
    }

    #[test]
    fn a_value_reads_back_and_a_missing_key_is_none() {
        let ks = Store::temp().unwrap().keyspace("a");
        assert_eq!(ks.get::<Doc>("k").unwrap(), None);
        ks.put("k", &Doc { n: 1 }).unwrap();
        assert_eq!(ks.get::<Doc>("k").unwrap(), Some(Doc { n: 1 }));
        assert!(ks.delete("k").unwrap());
        assert!(!ks.delete("k").unwrap());
        assert_eq!(ks.get::<Doc>("k").unwrap(), None);
    }

    #[test]
    fn keyspaces_of_one_store_do_not_see_each_other() {
        let store = Store::temp().unwrap();
        let (a, b) = (store.keyspace("a"), store.keyspace("b"));
        a.put("k", &Doc { n: 1 }).unwrap();
        a.index_put("idx", 1, "m").unwrap();
        assert_eq!(b.get::<Doc>("k").unwrap(), None);
        assert!(b.index_all("idx").unwrap().is_empty());
        assert!(!b.delete("k").unwrap());
        assert_eq!(a.get::<Doc>("k").unwrap(), Some(Doc { n: 1 }));
    }

    #[test]
    fn put_new_keeps_the_first_value() {
        let ks = Store::temp().unwrap().keyspace("a");
        assert!(ks.put_new("k", &Doc { n: 1 }).unwrap());
        assert!(!ks.put_new("k", &Doc { n: 2 }).unwrap());
        assert_eq!(ks.get::<Doc>("k").unwrap(), Some(Doc { n: 1 }));
    }

    #[test]
    fn an_index_orders_by_score_and_ranges_are_inclusive() {
        let ks = Store::temp().unwrap().keyspace("a");
        ks.index_put("idx", 30, "c").unwrap();
        ks.index_put("idx", 10, "a").unwrap();
        ks.index_put("idx", 20, "b").unwrap();
        assert_eq!(ks.index_all("idx").unwrap(), ["a", "b", "c"]);
        assert_eq!(ks.index_range("idx", 10, 20).unwrap(), ["a", "b"]);
        ks.index_put("idx", 40, "a").unwrap();
        assert_eq!(ks.index_all("idx").unwrap(), ["b", "c", "a"]);
        ks.index_remove("idx", "c").unwrap();
        assert_eq!(ks.index_len("idx").unwrap(), 2);
        assert_eq!(ks.index_max("idx").unwrap(), Some(40));
        assert_eq!(ks.index_max("none").unwrap(), None);
        assert_eq!(ks.index_trim("idx", 20).unwrap(), 1);
        assert_eq!(ks.index_all("idx").unwrap(), ["a"]);
    }

    #[test]
    fn data_survives_reopening_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        Store::open(dir.path())
            .unwrap()
            .keyspace("a")
            .put("k", &Doc { n: 7 })
            .unwrap();
        let reopened = Store::open(dir.path()).unwrap().keyspace("a");
        assert_eq!(reopened.get::<Doc>("k").unwrap(), Some(Doc { n: 7 }));
    }
}
