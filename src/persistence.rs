use std::env;

use rusqlite::{Connection, Result};

use crate::block::{HeaderMap, ResultMap};

#[derive(Debug)]
pub struct SQLitePersistence {
    sql_conn: Connection,
    sqlite_absolute_path: String
}

impl SQLitePersistence {
    pub fn new() -> anyhow::Result<(Self)> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH")?;
        let sql_conn = Connection::open(&sqlite_absolute_path)?;

        let db_exec_results = sql_conn.execute(
            "create table if not exists p2pk_utxo_block_aggregates (
                 block_height integer primary key,
                 date integer not null,
                 p2pk_utxo_count integer not null,
                 p2pk_utxo_value real not null
             )",
            [],
        )?;
        println!("p2pk_utxo_block_aggregates: table now exists at: {}", sqlite_absolute_path);
    
        Ok(SQLitePersistence{sql_conn, sqlite_absolute_path})

    }

    pub fn persist_block_aggregates(header_map: &HeaderMap,result_map: &ResultMap) -> anyhow::Result<()> {

        let result_map_read = result_map.read().unwrap();

        Ok(())
    }
}