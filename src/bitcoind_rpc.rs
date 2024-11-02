use std::env;

use anyhow::{anyhow, Result};
use bitcoin::{Address, Amount, BlockHash};

use bitcoin::hashes::sha256d::Hash;
use bitcoincore_rpc::json::{self, GetAddressInfoResult, ListUnspentResultEntry};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use log::debug;
use r2d2;

/// bitcoincore_rpc::Client is not threadsafe, so we need to manage connections in an r2d2 pool
/// Represents a connection to a Bitcoin Core RPC interface
#[derive(Debug)]
pub struct BitcoindRpcInfo {
    rpc_pool: r2d2::Pool<BitcoindConnectionManager>,
}

#[derive(Debug)]
struct BitcoindConnectionManager {
    url: String,
    auth: Auth,
}

impl r2d2::ManageConnection for BitcoindConnectionManager {
    type Connection = Client;
    type Error = bitcoincore_rpc::Error;

    fn connect(&self) -> Result<Client, Self::Error> {
        Client::new(&self.url, self.auth.clone())
    }

    fn is_valid(&self, _: &mut Client) -> Result<(), Self::Error> {
        // Optional: Add validation logic here
        Ok(())
    }

    fn has_broken(&self, _: &mut Client) -> bool {
        false
    }
}

impl BitcoindRpcInfo {
    pub fn new(pool_size: u32) -> Result<Self> {
        let url = env::var("BITCOIND_RPC_URL")
            .map_err(|e| anyhow!("Missing BITCOIND_RPC_URL environment variable: {}", e))?;
        
        let auth = match env::var("BITCOIND_RPC_COOKIE_PATH") {
            Ok(cookiefile) => Auth::CookieFile(cookiefile.into()),
            Err(_) => {
                eprintln!("BITCOIND_RPC_COOKIE_PATH not set, using BITCOIND_RPC_USER and BITCOIND_RPC_PASS");
                let user = env::var("BITCOIND_RPC_USER")
                    .map_err(|e| anyhow!("Missing BITCOIND_RPC_USER environment variable: {}", e))?;
                let pass = env::var("BITCOIND_RPC_PASS")
                    .map_err(|e| anyhow!("Missing BITCOIND_RPC_PASS environment variable: {}", e))?;
                Auth::UserPass(user, pass)
            }
        };
        debug!("Creating BitcoindConnectionManager with url: {} ; pool size: {}", url, pool_size);
        let manager = BitcoindConnectionManager { url, auth };
        let pool = r2d2::Pool::builder()
            .max_size(pool_size) // Adjust pool size as needed
            .build(manager)
            .map_err(|e| anyhow!("Failed to create connection pool: {}", e))?;

        Ok(BitcoindRpcInfo { rpc_pool: pool })
    }

    pub fn get_bitcoind_info_for_test_p2pk(
        &self,
        output_amount_btc: f64,
    ) -> Result<(ListUnspentResultEntry, GetAddressInfoResult, Address, Amount)> {
        // Get network relay fee
        let network_relay_fee = self.rpc_pool.get()?.get_network_info()
            .map_err(|e| anyhow!("Failed to get network info: {}", e))?
            .relay_fee;
        let output_tx_total = network_relay_fee.to_btc() + output_amount_btc;

        // Find suitable UTXO
        let unspent_vec = self.rpc_pool.get()?.list_unspent(Some(3), None, None, None, None)
            .map_err(|e| anyhow!("Failed to list unspent transactions: {}", e))?;

        let unspent_tx = unspent_vec
            .into_iter()
            .find(|utxo| utxo.amount.to_btc() > output_tx_total)
            .ok_or_else(|| anyhow!("No unspent txs have sufficient funds: {}", output_tx_total))?;

        // Get input UTXO address info
        let input_utxo_address = unspent_tx.address.clone()
            .ok_or_else(|| anyhow!("UTXO has no address"))?
            .assume_checked();
        
        let input_utxo_address_info = self.rpc_pool.get()?.get_address_info(&input_utxo_address)
            .map_err(|e| anyhow!("Failed to get address info: {}", e))?;

        // Get change address
        let change_addr = self.rpc_pool.get()?.get_raw_change_address(Some(json::AddressType::Bech32))
            .map_err(|e| anyhow!("Failed to get change address: {}", e))?
            .assume_checked();

        Ok((unspent_tx, input_utxo_address_info, change_addr, network_relay_fee))
    }

    pub fn get_block_height(&self, sha256d_hash: &Hash) -> Result<usize> {
        let hash = BlockHash::from_raw_hash(*sha256d_hash);
        let block_header = self.rpc_pool.get()?.get_block_header_info(&hash)?;
        Ok(block_header.height)
    }
}
