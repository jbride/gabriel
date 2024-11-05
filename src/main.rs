mod api;
mod bitcoind_rpc;
mod block;
mod p2pktx;
mod persistence;
mod tx;

use std::{
    env,
    fs::{File, OpenOptions},
    io::{Seek, Write},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use anyhow::{Ok, Result};
use api::AppState;
use bitcoin::{hashes::sha256d::Hash, Amount};
use bitcoind_rpc::BitcoindRpcInfo;
use block::{
    process_block, process_block_file, process_blocks_in_parallel, BlockAggregateOutput, Record,
};
use clap::{Parser, Subcommand};
use nom::AsBytes;
use tokio::sync::broadcast;
use zeromq::{Socket, SocketRecv};

use block::{HeaderMap, ResultMap, TxMap};
use indicatif::ProgressBar;

use axum::routing::get;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

const HEADER: &str = "Height,Block Hash,Date,Total P2PK addresses,Total P2PK coins\n";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Index(IndexArgs),
    SingleBlockFileEval(BlockFileEvalArgs),
    BlockAsyncEvalAndWebApp,
    GenerateP2PKTx(GenerateP2PKTxArgs),
}

#[derive(Parser, Debug)]
struct GenerateP2PKTxArgs {
    #[arg(short, long, default_value = "1.0 BTC")]
    output_amount_btc: String,
    #[arg(short, long)]
    extended_master_private_key: String,
}

#[derive(Parser, Debug)]
struct BlockFileEvalArgs {
    /// Bitcoin directory path
    #[arg(short, long)]
    block_file_absolute_path: PathBuf,
}

#[derive(Parser, Debug)]
struct IndexArgs {
    /// Bitcoin directory path
    #[arg(short, long)]
    input: PathBuf,

    /// CSV output file path
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Parser, Debug)]
struct GraphArgs {
    // Add arguments for the graph command if needed
}

/*
   Using Tokio runtime to support the following async functions:
   - Asynchronous web server operations (Axum)
   - Asynchronous ZeroMQ socket operations
   - Asynchronous SQLite persistence operations
   - Concurrent processing of blocks
   - Async broadcast of new blocks to Server Sent Events (SSE) stream
*/
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut xor_key_path = env::var("BLOCK_XOR_KEY_FILE_PATH").ok();
    if xor_key_path.is_none() {
        if let std::result::Result::Ok(bitcoind_data_dir) = env::var("BITCOIND_DATA_DIR") {
            let possible_xor_key_path = format!("{}/blocks/{}", bitcoind_data_dir, block::BLOCK_XOR_KEY_FILE_DEFAULT_NAME);
            if File::open(&possible_xor_key_path).is_ok() {
                xor_key_path = Some(possible_xor_key_path);
            }
        }
    }

    match &cli.command {
        Commands::SingleBlockFileEval(args) => {
            run_single_block_file_eval(args, &xor_key_path).await
        }
        Commands::Index(args) => run_index(args, &xor_key_path),
        Commands::BlockAsyncEvalAndWebApp => {
            evaluate_async_blocks_and_run_web_app(&xor_key_path).await
        }
        Commands::GenerateP2PKTx(args) => generate_p2pk_tx(args),
    }
}

async fn run_single_block_file_eval(
    args: &BlockFileEvalArgs,
    xor_key_path: &Option<String>,
) -> Result<()> {
    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();

    // Maps txid to tx value
    let tx_map: TxMap = Default::default();

    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();
    let pb = ProgressBar::new(1);

    let xor_key = if let Some(path) = xor_key_path {
        Some(block::get_xor_key(Some(&path))?)
    } else {
        None
    };

    let blocks_processed = process_block_file(
        &args.block_file_absolute_path,
        &pb,
        &result_map,
        &tx_map,
        &header_map,
        &xor_key,
    );
    println!(
        "block_file_absolute_path: {} ;  blocks processed = {}",
        &args.block_file_absolute_path.display(),
        blocks_processed
    );
    if blocks_processed < 1 {
        return Ok(());
    }

    let bitcoind_info = BitcoindRpcInfo::new(1)?;
    let sqlite_persistence = persistence::SQLitePersistence::new(1).await?;

    let h_binding = header_map.read().unwrap();
    let mut header_map_iter = h_binding.iter();
    while let Some(h_map_entry) = header_map_iter.next() {
        let record = {
            let r_binding = result_map.read().unwrap();
            r_binding.get(h_map_entry.1).cloned()
        };

        let h_map_entry = (*h_map_entry.0, *h_map_entry.1);
        let block_aggregate =
            get_block_aggregate_output(&bitcoind_info, &h_map_entry, &record.unwrap())?;
        match sqlite_persistence
            .persist_block_aggregates(&block_aggregate)
            .await
        {
            std::result::Result::Ok(_) => {}
            Err(e) => {
                eprintln!(
                    "Error persisting {}, error={}",
                    block_aggregate.block_hash_big_endian, e
                );
            }
        }
    }
    Ok(())
}

// TO-DO: Consider writing anaylsis of each block immediately to sqlite (rather than populating in-memory maps)
fn run_index(args: &IndexArgs, xor_key_path: &Option<String>) -> Result<()> {
    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();
    // Maps txid to tx value
    let tx_map: TxMap = Default::default();
    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();

    let xor_key = if let Some(path) = xor_key_path {
        Some(block::get_xor_key(Some(&path))?)
    } else {
        None
    };

    if let Err(e) =
        process_blocks_in_parallel(&args.input, &result_map, &tx_map, &header_map, &xor_key)
    {
        eprintln!("Failed to process blocks: {:?}", e);
    }
    let mut out: Vec<String> = vec![];
    let mut last_block_hash: [u8; 32] =
        hex::decode("4860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000")
            .unwrap()
            .try_into()
            .expect("slice with incorrect length"); // Genesis block
    let mut height = 0;
    let mut p2pk_addresses = 0;
    let mut p2pk_coins = 0.0;
    while let Some(next_block_hash) = header_map.read().unwrap().get(&last_block_hash) {
        // println!("Next block hash: {:?}", hex::encode(next_block_hash.1));
        let result_map_read = result_map.read().unwrap();
        let record = result_map_read.get(next_block_hash);
        if let Some(record) = record {
            let Record {
                date,
                p2pk_addresses_added,
                p2pk_sats_added,
                p2pk_addresses_spent,
                p2pk_sats_spent,
            } = &record;
            p2pk_addresses += p2pk_addresses_added;
            p2pk_addresses -= p2pk_addresses_spent;
            p2pk_coins += p2pk_sats_added.to_owned() as f64 / 100_000_000.0;
            p2pk_coins -= p2pk_sats_spent.to_owned() as f64 / 100_000_000.0;
            out.push(format!("{height},{date},{p2pk_addresses},{p2pk_coins}"));
        }
        height += 1;
        last_block_hash = *next_block_hash;
    }

    println!("Last block hash: {:?}", hex::encode(last_block_hash));
    println!("Height: {}", height);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&args.output)?;

    // When writing back to the file, ensure we start from the beginning
    file.seek(std::io::SeekFrom::Start(0))?;
    file.set_len(0)?; // Truncate the file

    file.write_all(HEADER.as_bytes())?;
    for line in &out {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}

/*
 * This function evaluates blocks from Bitcoind ZMQ socket and broadcasts the results
 * to the Server Sent Events (SSE) stream.
 */
async fn evaluate_async_blocks_and_run_web_app(xor_key_path: &Option<String>) -> Result<()> {
    let zmq_socket_url =
        env::var("ZMQ_SOCKET_URL").expect("ZMQ_SOCKET_URL environment variable must be set");

    println!("zmqpubrawblock_socket_url: {}", &zmq_socket_url);

    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();
    // Maps txid to tx value
    let tx_map: TxMap = Default::default();

    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();
    let pb = ProgressBar::new(1);

    // Connect and Subscribe to Bitcoind ZMQ socket
    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&zmq_socket_url)
        .await
        .expect(&format!("Failed to connect: {}", &zmq_socket_url));
    socket.subscribe("").await?;

    let bitcoind_info = BitcoindRpcInfo::new(1)?;
    let sqlite_persistence = persistence::SQLitePersistence::new(1).await?;

    // Create a broadcast channel for SSE events and start the API server
    let (tx, _rx) = broadcast::channel(100);

    run_apis_and_web_app(tx.clone()).await?;

    let xor_key = if let Some(path) = xor_key_path {
        Some(block::get_xor_key(Some(&path))?)
    } else {
        None
    };

    loop {
        let zmq_message = socket.recv().await?;

        let second_element = zmq_message.get(1);
        match second_element {
            Some(block_bytes) => {
                let u8_byte_array = block_bytes.as_bytes();
                let tx_count = process_block(
                    u8_byte_array,
                    &pb,
                    &result_map,
                    &tx_map,
                    &header_map,
                    false,
                    &xor_key,
                );
                println!(
                    "received block! byte length: {}; tx_count: {}",
                    u8_byte_array.len(),
                    tx_count
                );

                let h_map_entry = {
                    let h_binding = header_map.read().unwrap();
                    let (key, value) = h_binding.first_key_value().unwrap();
                    (*key, *value)
                };
                let record = {
                    let r_binding = result_map.read().unwrap();
                    r_binding.first_key_value().unwrap().1.clone()
                };

                let block_aggregate =
                    get_block_aggregate_output(&bitcoind_info, &h_map_entry, &record)?;
                match sqlite_persistence
                    .persist_block_aggregates(&block_aggregate)
                    .await
                {
                    std::result::Result::Ok(_) => {}
                    Err(e) => {
                        eprintln!(
                            "Error persisting {}, error={}",
                            block_aggregate.block_hash_big_endian, e
                        );
                    }
                };

                // Broadcast the new block aggregate
                let _ = tx.send(block_aggregate);
            }
            None => panic!("second element from zeromq raw block is non-existent!"),
        }
    }
}

async fn run_apis_and_web_app(tx: broadcast::Sender<BlockAggregateOutput>) -> Result<()> {
    // Create a SQLite persistence instance with a connection pool
    let sqlite_persistence = persistence::SQLitePersistence::new(5).await?;

    let app_state = Arc::new(AppState {
        db: sqlite_persistence,
        tx: tx,
    });

    // Define routes for REST API, SSE stream, and React frontend
    let web_app_router = axum::Router::new()
        .route("/api/aggregates", get(api::get_aggregates))
        .route("/api/block/hash/:hash", get(api::get_block_by_hash))
        .route("/api/block/height/:height", get(api::get_block_by_height))
        .route("/api/blocks/stream", get(api::stream_blocks))
        .nest_service("/", ServeDir::new("web/build"))
        .with_state(app_state);

    // Determine socket that web_app will bind to
    let web_addr: SocketAddr = env::var("WEB_SOCKET_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()
        .expect("Failed to parse API_ADDR");
    println!("REST API listening on {}", web_addr);

    // Spawn the web app server in the background
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(web_addr).await.unwrap();
        axum::serve(listener, web_app_router.into_make_service())
            .await
            .unwrap();
    });

    Ok(())
}

fn generate_p2pk_tx(args: &GenerateP2PKTxArgs) -> Result<()> {
    let to_amount = Amount::from_str(&args.output_amount_btc)?;
    let e_master_key = &args.extended_master_private_key;
    p2pktx::generate_p2pk_tx(e_master_key, to_amount)
}

fn get_block_aggregate_output(
    bitcoind_info: &BitcoindRpcInfo,
    h_map_entry: &([u8; 32], [u8; 32]),
    record: &Record,
) -> Result<BlockAggregateOutput> {
    // Get previous and current block hashes
    let mut raw_previous_block_hash = h_map_entry.0.clone();
    raw_previous_block_hash.reverse();
    let previous_block_hash = hex::encode(raw_previous_block_hash);

    let mut raw_current_block_hash = h_map_entry.1.clone();
    let sha256d_hash = Hash::from_bytes_ref(&raw_current_block_hash);

    let mut block_height = 0;

    match bitcoind_info.get_block_height(sha256d_hash) {
        std::result::Result::Ok(x) => block_height = x,
        Err(e) => println!("block not found: exception={}", e),
    }
    raw_current_block_hash.reverse();
    let current_block_hash_header = hex::encode(raw_current_block_hash);

    println!(
        "previous_block_hash={} , current_block_hash={}, block_height={}",
        previous_block_hash, current_block_hash_header, block_height
    );

    // Determine total p2pk addresses and value
    let mut total_p2pk_addresses = record.p2pk_addresses_added.to_owned();
    total_p2pk_addresses -= record.p2pk_addresses_spent.to_owned();
    let mut total_p2pk_value = record.p2pk_sats_added.to_owned() as f64 / 100_000_000.0;
    total_p2pk_value -= record.p2pk_sats_spent.to_owned() as f64 / 100_000_000.0;
    let date = &record.date;

    Ok(BlockAggregateOutput {
        date: date.clone(),
        block_height,
        block_hash_big_endian: current_block_hash_header,
        total_p2pk_addresses,
        total_p2pk_value,
    })
}
