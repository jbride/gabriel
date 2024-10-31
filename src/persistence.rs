use std::env;

use log::debug;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::block::BlockAggregateOutput;

#[derive(Debug)]
pub struct SQLitePersistence {
    pool: Pool<SqliteConnectionManager>
}

impl SQLitePersistence {
    pub fn new() -> anyhow::Result<(Self)> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH")
            .map_err(|e| anyhow::anyhow!("Missing SQLITE_ABSOLUTE_PATH environment variable: {}", e))?;
        let manager = SqliteConnectionManager::file(&sqlite_absolute_path);
        let pool = r2d2::Pool::builder().max_size(15).build(manager)?;

        let sql_conn = pool.get()?;
        sql_conn.execute(
            "create table if not exists p2pk_utxo_block_aggregates (
                 block_height integer not null,
                 block_hash_big_endian text primary key,
                 date text not null,
                 total_p2pk_addresses integer not null,
                 total_p2pk_value real not null
             )",
            [],
        )?;
        debug!(
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

    pub fn get_total_aggregates(&self) -> anyhow::Result<(i64, f64)> {
        let sql_conn = self.pool.get().unwrap();
        let mut stmt = sql_conn.prepare(
            "SELECT SUM(total_p2pk_addresses) as total_count, 
             SUM(total_p2pk_value) as total_value 
             FROM p2pk_utxo_block_aggregates"
        )?;
        
        let (count, value) = stmt.query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        
        Ok((count, value))
    }

    pub fn get_block_by_hash(&self, hash: &str) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let sql_conn = self.pool.get().unwrap();
        let mut stmt = sql_conn.prepare(
            "SELECT date, block_height, block_hash_big_endian, total_p2pk_addresses, total_p2pk_value 
             FROM p2pk_utxo_block_aggregates WHERE block_hash_big_endian = ?"
        )?;
        
        let result = stmt.query_row([hash], |row| {
            Ok(BlockAggregateOutput {
                date: row.get(0)?,
                block_height: row.get(1)?,
                block_hash_big_endian: row.get(2)?,
                total_p2pk_addresses: row.get(3)?,
                total_p2pk_value: row.get(4)?,
            })
        });
        
        match result {
            Ok(block) => Ok(Some(block)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_block_by_height(&self, height: i64) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let sql_conn = self.pool.get().unwrap();
        let mut stmt = sql_conn.prepare(
            "SELECT date, block_height, block_hash_big_endian, total_p2pk_addresses, total_p2pk_value 
             FROM p2pk_utxo_block_aggregates WHERE block_height = ?"
        )?;
        
        let result = stmt.query_row([height], |row| {
            Ok(BlockAggregateOutput {
                date: row.get(0)?,
                block_height: row.get(1)?,
                block_hash_big_endian: row.get(2)?,
                total_p2pk_addresses: row.get(3)?,
                total_p2pk_value: row.get(4)?,
            })
        });
        
        match result {
            Ok(block) => Ok(Some(block)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
