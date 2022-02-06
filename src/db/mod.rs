use rusqlite::{params, Connection, Result};
use std::path::{Path,PathBuf};
use std::fs::create_dir_all;

// TODO might need to mv all const to common location
const IDUP_DIR_NAME: &str = "idup";
const IDUP_DB_NAME: &str = "idup.db3";

pub fn save(file_name: &str, sha256: &str, phash: u64) {
    let conn = open_db().unwrap();
    conn.execute(
        "INSERT INTO hashed (path, sha256, phash) values (?1, ?2, ?3)",
        params![file_name, sha256, phash.to_string()],
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS hashed (
            path TEXT PRIMARY KEY,
            sha256 TEXT,
            phash TEXT
        )",
        [],
    )?;
    Ok(())
}
