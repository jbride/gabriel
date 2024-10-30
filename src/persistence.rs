use std::env;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::block::BlockAggregateOutput;

#[derive(Debug)]
pub struct SQLitePersistence {
    pool: Pool<SqliteConnectionManager>
}

impl SQLitePersistence {
    pub fn new() -> anyhow::Result<(Self)> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH")?;
        let manager = SqliteConnectionManager::file(&sqlite_absolute_path);
        let pool = r2d2::Pool::builder().max_size(15).build(manager).unwrap();

        let sql_conn = pool.get().unwrap();
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
            pool
        })
    }

    pub fn persist_block_aggregates(&self, block_aggregate: &BlockAggregateOutput) -> anyhow::Result<(usize)> {
        
        let sql = "INSERT INTO p2pk_utxo_block_aggregates VALUES(?1,?2,?3,?4,?5)";

        let sql_conn = self.pool.get().unwrap();
        let db_exec_results = sql_conn.execute(sql, [
            block_aggregate.block_height.to_string(),
            block_aggregate.block_hash_big_endian.clone(),
            block_aggregate.date.clone(),
            block_aggregate.total_p2pk_addresses.to_string(),
            block_aggregate.total_p2pk_value.to_string()
        ])?;

        Ok(db_exec_results)
    }
}
