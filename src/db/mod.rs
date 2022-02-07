use rusqlite::{params, Connection, Result};
use std::path::{Path,PathBuf};
use std::fs::create_dir_all;
use std::vec::Vec;

// TODO might need to mv all const to common location
const IDUP_DIR_NAME: &str = "idup";
const IDUP_DB_NAME: &str = "idup.db3";

#[derive(Debug)]
pub struct ImgData {
    pub path: PathBuf,
    pub sha256: String,
    pub phash: String
}

pub fn exact_match(path: &PathBuf) -> Result<Vec<ImgData>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("
        SELECT path, sha256, phash
        FROM hashed
        WHERE sha256 in (SELECT sha256 FROM hashed WHERE path = ?1)
        ;"
    )?;
    let iter = stmt.query_map([path.to_str()], |row| {
        let s: String = row.get(0)?;
        Ok(ImgData {
            path: Path::new(&s).to_path_buf(),
            sha256: row.get(1)?,
            phash: row.get(2)?,
        })
    })?;
    iter.collect()
}

pub fn exact_matches() -> Result<Vec<ImgData>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("
        SELECT a.path, a.sha256, a.phash, b.cnt
        FROM hashed a
        JOIN (
          SELECT sha256, count(*) as cnt
          FROM hashed
          GROUP BY sha256
          HAVING cnt > 1
        ) b
          ON a.sha256 = b.sha256
        ORDER BY b.cnt DESC
        ;"
    )?;
    let iter = stmt.query_map([], |row| {
        let s: String = row.get(0)?;
        Ok(ImgData {
            path: Path::new(&s).to_path_buf(),
            sha256: row.get(1)?,
            phash: row.get(2)?,
        })
    })?;
    iter.collect()
}

pub fn save(i: &ImgData) {
    let conn = open_db().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO hashed (path, sha256, phash) values (?1, ?2, ?3)",
        params![i.path.to_str(), i.sha256, i.phash.to_string()],
    ).unwrap();
}

fn open_db() -> Result<Connection> {
    let db_path = setup_dir();
    let conn = Connection::open(db_path)?;
    setup_db(&conn)?;
    Ok(conn)
}

fn setup_dir() -> PathBuf {
    // TODO this should respect XDG_DATA_HOME
    let db_path = Path::new("/home/cd/.local/share").join(IDUP_DIR_NAME).join(IDUP_DB_NAME);
    let parent = db_path.parent().unwrap();
    create_dir_all(parent).unwrap();
    db_path.to_path_buf()
}

fn setup_db(conn: &Connection) -> Result<()> {
    conn.execute("
        CREATE TABLE IF NOT EXISTS hashed (
          path TEXT PRIMARY KEY,
          sha256 TEXT,
          phash TEXT
        )",
        [],
    )?;
    Ok(())
}
