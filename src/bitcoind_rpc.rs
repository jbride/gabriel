use std::env;

use anyhow::{anyhow, Result};
use bitcoin::{Address, Amount, BlockHash};

use bitcoin::hashes::sha256d::Hash;
use bitcoincore_rpc::json::{self, GetAddressInfoResult, ListUnspentResultEntry};
use bitcoincore_rpc::{Auth, Client, RpcApi};

#[derive(Debug)]
pub struct BitcoindRpcInfo {
    rpc_client: Client,
}

impl BitcoindRpcInfo {
    pub fn new() -> Result<Self> {
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
        let rpc_client = Client::new(&url, auth)?;
        Ok(BitcoindRpcInfo { rpc_client })
    }

    pub fn get_bitcoind_info_for_test_p2pk(
        &self,
        output_amount_btc: f64,
    ) -> Result<(
        ListUnspentResultEntry,
        GetAddressInfoResult,
        Address,
        Amount,
    )> {
        // 1)  get default bitcoind relay_fee
        let network_relay_fee = self.rpc_client.get_network_info()?.relay_fee;
        let output_tx_total = network_relay_fee.to_btc() + output_amount_btc;

        // 2)  identify first utxo managed by bitcoind wallet with a value > desired p2pk output
        let mut unspent_option: Option<ListUnspentResultEntry> = None;
        let unspent_vec = self
            .rpc_client
            .list_unspent(Some(3), None, None, None, None)
            .unwrap();
        for unspent_candidate in unspent_vec {
            println!(
                "unspent_candidate txid={}, vout={}, tx_amount={}, output_tx_total={}",
                unspent_candidate.txid,
                unspent_candidate.vout,
                unspent_candidate.amount.to_btc(),
                output_tx_total
            );
            if unspent_candidate.amount.to_btc() > output_tx_total {
                unspent_option = Some(unspent_candidate);
                break;
            }
        }
        if unspent_option == None {
            return Err(anyhow!(
                "No unspent txs have sufficient funds: {}",
                output_tx_total
            ));
        }

        let unspent_tx = unspent_option.unwrap();
        let input_utxo_address = unspent_tx.address.clone().unwrap().assume_checked();

        // 3) Get info about the input utxo address
        let input_utxo_address_info = self.rpc_client.get_address_info(&input_utxo_address)?;

        // 4)
        let change_addr = self
            .rpc_client
            .get_raw_change_address(Some(json::AddressType::Bech32))
            .unwrap()
            .assume_checked();

        Ok((
            unspent_tx,
            input_utxo_address_info,
            change_addr,
            network_relay_fee,
        ))
    }

    pub fn get_block_height(&self, sha256d_hash: &Hash) -> Result<usize> {
        let hash = BlockHash::from_raw_hash(*sha256d_hash);
        let block_header = self.rpc_client.get_block_header_info(&hash)?;
        Ok(block_header.height)
    }
}
