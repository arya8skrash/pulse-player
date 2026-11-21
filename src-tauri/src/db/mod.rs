mod models;
mod queries;

pub use models::{AlbumRow, ArtistRow, Folder, GenreRow, NewTrack, Playlist, Track, TrackQuery};

use std::{path::Path, sync::{Mutex, MutexGuard}};

use rusqlite::Connection;

use crate::error::AppResult;

const MIGRATIONS: &[&str] = &[
    include_str!("schema.sql"), 
    include_str!("schema_v2.sql"), 
    include_str!("schema_v3.sql"),
    include_str!("schema_v4.sql"), 
];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        let db = Db { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> AppResult<()> {
        let mut conn = self.lock();
        let current: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            if i == 2 {
                let columns: Vec<String> = conn
                    .prepare("PRAGMA table_info(folders)")?
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<_, _>>()?;
                
                if columns.contains(&"is_hidden".to_string()) {
                    conn.pragma_update(None, "user_version", (i + 1) as i64)?;
                    log::info!("migration 3 skipped (column already exists)");
                    continue;
                }
            }
            
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
            log::info!("applied migration {}", i + 1);
        }
        Ok(())
    }
}