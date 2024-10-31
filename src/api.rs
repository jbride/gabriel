use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use crate::persistence::SQLitePersistence;
use std::sync::Arc;

#[derive(Serialize)]
pub struct AggregateResponse {
    total_p2pk_utxo_count: i64,
    total_p2pk_utxo_value: f64,
}

#[derive(Serialize)]
pub struct BlockResponse {
    date: String,
    block_height: usize,
    block_hash: String,
    total_p2pk_addresses: u32,
    total_p2pk_value: f64,
}

pub struct AppState {
    pub(crate) db: SQLitePersistence,
}

pub async fn get_aggregates(
    State(state): State<Arc<AppState>>
) -> Json<AggregateResponse> {
    let (count, value) = state.db.get_total_aggregates().unwrap();
    
    Json(AggregateResponse {
        total_p2pk_utxo_count: count,
        total_p2pk_utxo_value: value,
    })
}

pub async fn get_block_by_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_hash(&hash).unwrap();
    
    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_p2pk_addresses: b.total_p2pk_addresses as u32,
        total_p2pk_value: b.total_p2pk_value,
    }))
}

pub async fn get_block_by_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<i64>
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_height(height).unwrap();
    
    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_p2pk_addresses: b.total_p2pk_addresses as u32,
        total_p2pk_value: b.total_p2pk_value,
    }))
} 