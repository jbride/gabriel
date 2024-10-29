use std::env;

use rusqlite::Connection;

use crate::block::BlockAggregateOutput;

#[derive(Debug)]
pub struct SQLitePersistence {
    sql_conn: Connection
}

impl SQLitePersistence {
    pub fn new() -> anyhow::Result<(Self)> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH")?;
        let sql_conn = Connection::open(&sqlite_absolute_path)?;

        sql_conn.execute(
            "create table if not exists p2pk_utxo_block_aggregates (
                 block_height integer not null,
                 block_hash text primary key,
                 date text not null,
                 p2pk_utxo_count integer not null,
                 p2pk_utxo_value real not null
             )",
            [],
        )?;
        println!(
            "p2pk_utxo_block_aggregates: table now exists at: {}",
            sqlite_absolute_path
        );

        Ok(SQLitePersistence {
            sql_conn
        })
    }

    pub fn persist_block_aggregates(&self, block_aggregate: &BlockAggregateOutput) -> anyhow::Result<(usize)> {
        
        let sql = "INSERT INTO p2pk_utxo_block_aggregates VALUES(?1,?2,?3,?4,?5)";
        let db_exec_results = self.sql_conn.execute(sql, [
            block_aggregate.block_height.to_string(),
            block_aggregate.block_hash_big_endian.clone(),
            block_aggregate.date.clone(),
            block_aggregate.total_p2pk_addresses.to_string(),
            block_aggregate.total_p2pk_value.to_string()
        ])?;

        Ok(db_exec_results)
    }
}
