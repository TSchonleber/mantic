use crate::error::Result;
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct BrainStatus {
    pub memory_count: u64,
    pub event_count: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct EventSummary {
    pub id: i64,
    pub event_type: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct MemorySummary {
    pub id: i64,
    pub category: String,
    pub content: String,
    pub created_at: i64,
}

pub struct BrainDb {
    conn: Mutex<Connection>,
}

impl BrainDb {
    /// Open the brain.db file at the given path, read-only.
    /// File must already exist (brainctl-mcp handles creation).
    pub fn open_read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn status(&self) -> Result<BrainStatus> {
        let conn = self.conn.lock().unwrap();
        let memory_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
        let event_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        Ok(BrainStatus {
            memory_count,
            event_count,
        })
    }

    pub fn recent_events(&self, limit: u32) -> Result<Vec<EventSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, event_type, content, created_at FROM events ORDER BY created_at DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(EventSummary {
                    id: r.get(0)?,
                    event_type: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn recent_memories(&self, limit: u32) -> Result<Vec<MemorySummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, content, created_at FROM memories ORDER BY created_at DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(MemorySummary {
                    id: r.get(0)?,
                    category: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Build a minimal fixture brain.db with the columns BrainDb reads.
    /// We don't mirror brainctl's full schema — just what the read methods touch.
    fn fixture_db() -> NamedTempFile {
        let f = NamedTempFile::new().unwrap();
        let path = f.path();
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE memories (
                id INTEGER PRIMARY KEY,
                category TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE events (
                id INTEGER PRIMARY KEY,
                event_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            INSERT INTO memories (category, content, created_at) VALUES
                ('lesson', 'first', 100),
                ('decision', 'second', 200),
                ('preference', 'third', 300);
            INSERT INTO events (event_type, content, created_at) VALUES
                ('observation', 'ev1', 1000),
                ('result', 'ev2', 1100);
            "#,
        )
        .unwrap();
        f
    }

    #[test]
    fn status_returns_counts() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let s = db.status().unwrap();
        assert_eq!(s.memory_count, 3);
        assert_eq!(s.event_count, 2);
    }

    #[test]
    fn recent_events_orders_by_created_at_desc() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let events = db.recent_events(10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].content, "ev2");
        assert_eq!(events[1].content, "ev1");
    }

    #[test]
    fn recent_memories_respects_limit() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let mems = db.recent_memories(2).unwrap();
        assert_eq!(mems.len(), 2);
        assert_eq!(mems[0].content, "third");
        assert_eq!(mems[1].content, "second");
    }

    #[test]
    fn open_nonexistent_file_returns_error() {
        let r = BrainDb::open_read_only(Path::new("/nonexistent/brain.db"));
        assert!(r.is_err());
    }

    #[test]
    fn read_only_mode_blocks_writes() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let conn = db.conn.lock().unwrap();
        let r = conn.execute("INSERT INTO memories (category, content, created_at) VALUES ('x', 'y', 0)", []);
        assert!(r.is_err(), "expected write to be rejected on read-only connection");
    }
}
