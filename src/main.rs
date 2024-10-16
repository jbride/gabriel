use std::{
    fs::{OpenOptions, File},
    io::{Read, Seek, Write},
    path::PathBuf,
};

use anyhow::{Ok, Result};
use block::{process_block, process_block_file, process_blocks_in_parallel, Record};
use clap::{Parser, Subcommand};
use nom::AsBytes;
use zeromq::{Socket, SocketRecv};

mod block;
mod tx;

use block::{HeaderMap, ResultMap, TxMap};
use indicatif::ProgressBar;

const HEADER: &str = "Height,Date,Total P2PK addresses,Total P2PK coins\n";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    BlockFileEval(BlockFileEvalArgs),
    BlockAsyncEval(BlockAsyncEvalArgs),
    Index(IndexArgs),
    Graph(GraphArgs),
}

#[derive(Parser, Debug)]
struct BlockFileEvalArgs {
    /// Bitcoin directory path
    #[arg(short, long)]
    block_file_absolute_path: PathBuf,

    /// CSV output file path
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Parser, Debug)]
struct BlockAsyncEvalArgs {
    /// zmqpubrawblock  socket URL
    #[arg(short, long)]
    zmqpubrawblock_socket_url: String,

    /// CSV output file path
    #[arg(short, long)]
    output: PathBuf,
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::BlockFileEval(args) => run_block_file_eval(args),
        Commands::Index(args) => run_index(args),
        Commands::Graph(args) => run_graph(args),
        Commands::BlockAsyncEval(args) => run_async_block_eval_listener(args).await
    }
}

fn append_to_output(mut file: &File, result_map: &ResultMap) -> Result<()> {
    let result_map_read = result_map.read().unwrap();
    for (_key, record) in result_map_read.iter() {
        // write a record to the file
        let mut p2pk_addresses = &record.p2pk_addresses_added;
        let binding = p2pk_addresses - &record.p2pk_addresses_spent;
        p2pk_addresses = &binding;
        let mut p2pk_coins = record.p2pk_sats_added.to_owned() as f64 / 100_000_000.0;
        p2pk_coins -= record.p2pk_sats_spent.to_owned() as f64 / 100_000_000.0;
        let date = &record.date;
        let output_line = format!("0,{date},{p2pk_addresses},{p2pk_coins}");
        writeln!(file, "{}", output_line)?;
    }
    Ok(())

}

fn run_block_file_eval(args: &BlockFileEvalArgs) -> Result<()> {


    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();

    // Maps txid to tx value
    let tx_map: TxMap = Default::default();

    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();
    let pb = ProgressBar::new(1);

    let blocks_processed = process_block_file(
        &args.block_file_absolute_path,
        &pb,
        &result_map,
        &tx_map,
        &header_map,
    );
    println!(
        "block_file_absolute_path: {} ;  blocks processed = {}",
        &args.block_file_absolute_path.display(),
        blocks_processed
    );
    if blocks_processed < 1 {
        return Ok(());
    }

    // prep output file
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

    append_to_output(&file, &result_map);

    Ok(())
}

async fn run_async_block_eval_listener(args: &BlockAsyncEvalArgs) -> Result<()> {

    println!(
        "zmqpubrawblock_socket_url: {} ;  output file = {}",
        &args.zmqpubrawblock_socket_url,
        &args.output.display()
    );

    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();
    // Maps txid to tx value
    let tx_map: TxMap = Default::default();

    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();
    let pb = ProgressBar::new(1);

    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&args.zmqpubrawblock_socket_url)
        .await
        .expect(&format!("Failed to connect: {}", &args.zmqpubrawblock_socket_url));

    socket.subscribe("").await?;

    // prep output file
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&args.output)?;

    loop {
        let zmq_message = socket.recv().await?;
        
        let second_element = zmq_message.get(1);
        match second_element {
            Some(block_bytes) => {
                let u8_byte_array = block_bytes.as_bytes();
                let tx_count = process_block(u8_byte_array, &pb, &result_map, &tx_map, &header_map, false);
                println!("received block! byte length: {}; tx_count: {}", u8_byte_array.len(), tx_count);
                let _ = append_to_output(&file, &result_map);
            }
            None => panic!("second element from zeromq raw block is non-existent!")
        }
    }
}

fn run_index(args: &IndexArgs) -> Result<()> {
    // Maps previous block hash to next merkle root
    let header_map: HeaderMap = Default::default();
    // Maps txid to tx value
    let tx_map: TxMap = Default::default();
    // Maps header hash to result Record
    let result_map: ResultMap = Default::default();

    if let Err(e) = process_blocks_in_parallel(&args.input, &result_map, &tx_map, &header_map) {
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

fn run_graph(_args: &GraphArgs) -> Result<()> {
    // TODO: Implement graph functionality
    println!("Graph functionality not yet implemented");
    Ok(())
}
