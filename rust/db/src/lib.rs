use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

const TRANSACTION_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("i/o error")]
    Io(#[from] std::io::Error),
    #[error("sqlite error")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct EarthDb {
    connection_string: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record<T> {
    pub value: T,
    pub version: i64,
}

impl EarthDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let connection_string = path.as_ref().to_path_buf();
        if let Some(parent) = connection_string.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let connection = open_connection(&connection_string)?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS objects (
                type TEXT NOT NULL,
                id TEXT NOT NULL,
                value TEXT NOT NULL,
                version INTEGER NOT NULL,
                PRIMARY KEY (type, id)
            )",
            [],
        )?;

        Ok(Self { connection_string })
    }

    pub fn get<T>(&self, object_type: &str, id: &str) -> Result<Record<T>, DatabaseError>
    where
        T: Default + DeserializeOwned,
    {
        let connection = open_connection(&self.connection_string)?;
        let mut statement =
            connection.prepare("SELECT value, version FROM objects WHERE type = ?1 AND id = ?2")?;
        let mut rows = statement.query(params![object_type, id])?;

        if let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            let version: i64 = row.get(1)?;
            Ok(Record {
                value: serde_json::from_str(&value)?,
                version,
            })
        } else {
            Ok(Record {
                value: T::default(),
                version: 1,
            })
        }
    }

    pub fn update<T>(&self, object_type: &str, id: &str, value: &T) -> Result<i64, DatabaseError>
    where
        T: Serialize,
    {
        let connection = open_connection(&self.connection_string)?;
        let tx = connection.unchecked_transaction()?;
        let value = serde_json::to_string(value)?;

        tx.execute(
            "INSERT INTO objects(type, id, value, version)
             VALUES (?1, ?2, ?3, 2)
             ON CONFLICT(type, id)
             DO UPDATE SET value = excluded.value, version = objects.version + 1",
            params![object_type, id, value],
        )?;

        let version: i64 = tx.query_row(
            "SELECT version FROM objects WHERE type = ?1 AND id = ?2",
            params![object_type, id],
            |row| row.get(0),
        )?;

        tx.commit()?;
        Ok(version)
    }

    pub fn bump<T>(&self, object_type: &str, id: &str) -> Result<Record<T>, DatabaseError>
    where
        T: Default + Serialize + DeserializeOwned,
    {
        let current = self.get::<T>(object_type, id)?;
        let value = if current.version == 1 {
            T::default()
        } else {
            current.value
        };
        let version = self.update(object_type, id, &value)?;
        Ok(Record { value, version })
    }
}

fn open_connection(path: &Path) -> Result<Connection, DatabaseError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(std::time::Duration::from_millis(TRANSACTION_TIMEOUT_MS))?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    use super::EarthDb;

    #[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
    struct Example {
        value: i32,
    }

    #[test]
    fn round_trip_and_bump() {
        let temp_dir = tempdir().expect("tempdir should exist");
        let db = EarthDb::open(temp_dir.path().join("earth.db")).expect("db should open");

        let initial = db.get::<Example>("example", "user-1").expect("get should work");
        assert_eq!(initial.version, 1);
        assert_eq!(initial.value, Example::default());

        let version = db
            .update("example", "user-1", &Example { value: 7 })
            .expect("update should work");
        assert_eq!(version, 2);

        let current = db.get::<Example>("example", "user-1").expect("get should work");
        assert_eq!(current.value, Example { value: 7 });
        assert_eq!(current.version, 2);

        let bumped = db.bump::<Example>("example", "user-1").expect("bump should work");
        assert_eq!(bumped.version, 3);
    }
}
