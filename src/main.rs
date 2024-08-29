use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
};

use anyhow::Result;
use bitcoincore_rpc::{
    json::{GetChainTipsResultStatus, GetChainTipsResultTip},
    Auth, Client, RpcApi,
};
use chrono::{TimeZone, Utc};
use indicatif::ProgressBar;

const HEADER: &str = "Height,Date,Total P2PK addresses,Total P2PK coins";

fn main() -> Result<()> {
    let mut out: Vec<String> = vec![];
    out.push(HEADER.to_owned());

    // Open the file if it exists, otherwise create it and write the HEADER.
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("out.csv")?;

    // Check if the file is empty by checking its length
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let lines = content.split("\n").collect::<Vec<&str>>();
    if lines.is_empty() {
        out.push(HEADER.to_owned());
    }

    // Rewind the file to the beginning so you can read from it again
    file.rewind()?;

    // Read the file content into a vector of strings
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    out = content.lines().map(|line| line.to_string()).collect();

    // Get the last line of the CSV file and parse the height from it
    let resume_height = if let Some(last_line) = out.last() {
        let fields: Vec<&str> = last_line.split(',').collect();
        if let Some(height_str) = fields.first() {
            height_str.parse::<u64>().unwrap_or(1)
        } else {
            1
        }
    } else {
        1
    };

    // If the file only contains the header, set the resume height to 1
    let resume_height = if resume_height == 0 { 1 } else { resume_height };

    // Get the last line of the CSV file and parse the P2PK addresses and coins from it
    let mut p2pk_addresses: i32 = if let Some(last_line) = out.last() {
        let fields: Vec<&str> = last_line.split(',').collect();
        if fields.len() >= 3 {
            fields[2].parse().unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };
    let mut p2pk_coins: f64 = if let Some(last_line) = out.last() {
        let fields: Vec<&str> = last_line.split(',').collect();
        if fields.len() >= 4 {
            fields[3].parse().unwrap_or(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // RPC connection
    let url = env::var("URL")?;
    let cookie = env::var("COOKIE");
    let auth = match cookie {
        Ok(cookiefile) => Auth::CookieFile(cookiefile.into()),
        Err(_) => {
            let user = env::var("USER")?;
            let pass = env::var("PASS")?;

            Auth::UserPass(user, pass)
        }
    };
    let rpc = Client::new(&url, auth)?;

    // Get chain height from chain tip
    let result = rpc.get_chain_tips()?;
    let tip_height = result
        .iter()
        .filter(|fork: &&GetChainTipsResultTip| fork.status == GetChainTipsResultStatus::Active)
        .collect::<Vec<_>>()
        .first()
        .unwrap()
        .height;

    // Progress bar
    let pb = ProgressBar::new(tip_height);
    pb.inc(resume_height - 1);
    pb.println(format!(
        "Syncing from blocks {resume_height} to {tip_height}"
    ));

    // For each block, account for P2PK coins
    for height in resume_height..tip_height {
        let hash = rpc.get_block_hash(height)?;
        let block = rpc.get_block(&hash)?;

        // Account for the new P2PK coins
        for (i, tx) in block.txdata.iter().enumerate() {
            for outpoint in &tx.output {
                if outpoint.script_pubkey.is_p2pk() {
                    p2pk_addresses += 1;
                    p2pk_coins += outpoint.value.to_btc();
                }
            }

            // If the transaction is not from the coinbase, account for the spent coins
            if i > 1 {
                for txin in &tx.input {
                    let txid = txin.previous_output.txid;
                    let transaction = rpc.get_raw_transaction(&txid, None)?;

                    if transaction.is_coinbase() {
                        continue;
                    }

                    pb.println(format!("{height}: {transaction:?}"));

                    // Account for the spent P2PK coins
                    for outpoint in transaction.output {
                        if outpoint.script_pubkey.is_p2pk() {
                            p2pk_addresses -= 1;
                            p2pk_coins -= outpoint.value.to_btc();
                        }
                    }
                }
            }
        }

        // Format block header timestamp
        let datetime = Utc
            .timestamp_opt(block.header.time as i64, 0)
            .single()
            .expect("Invalid timestamp");

        let formatted_date = datetime.format("%m/%d/%Y %H:%M:%S").to_string();

        // Append the new line to the CSV file
        out.push(format!(
            "{height},{formatted_date},{p2pk_addresses},{p2pk_coins}",
        ));

        // Calculate ETA
        let eta_duration = pb.eta();
        let eta_seconds = eta_duration.as_secs();
        let days = eta_seconds / 86400;
        let hours = (eta_seconds % 86400) / 3600;
        let minutes = (eta_seconds % 3600) / 60;
        let seconds = eta_seconds % 60;
        let eta = format!("{:02}:{:02}:{:02}:{:02}", days, hours, minutes, seconds);

        pb.println(format!("Block: {height} - ETA: {eta}"));

        // Write the new content to the file for every 1000 blocks
        if height % 1000 == 0 {
            let content = out.join("\n");
            let mut file = File::create("out.csv")?;
            file.write_all(content.as_bytes())?;
            pb.println("FILE SUCCESSFULLY SAVED TO DISK");
        }

        pb.inc(1);
    }

    Ok(())
}
