//! Implements an example PSBT workflow.
//!
//! The workflow we simulate is that of a setup using a watch-only online wallet (contains only
//! public keys) and a cold-storage signing wallet (contains the private keys).
//!

//!

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use anyhow::{Ok, Result, anyhow};

use bitcoin::bip32::{self, ChildNumber, DerivationPath, Fingerprint, IntoDerivationPath, Xpriv, Xpub};
use bitcoin::consensus::encode;
use bitcoin::key::rand;
use bitcoin::locktime::absolute;
use bitcoin::psbt::{self, Input, Psbt, PsbtSighashType};
use bitcoin::secp256k1::{Secp256k1, Signing, Verification};
use bitcoin::{
    key, transaction, Address, Amount, CompressedPublicKey, Network, OutPoint, PrivateKey, PublicKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness
};

extern crate bitcoincore_rpc;
use bitcoincore_rpc::json::{self, GetAddressInfoResult, ListUnspentResultEntry};
use bitcoincore_rpc::{Auth, Client, RpcApi};

const INPUT_UTXO_VOUT: u32 = 0;

#[derive(Debug)]
pub struct BitcoindRpcInfo {
    pub rpc_url: String,
    pub rpc_user_id: String,
    pub rpc_password: String
}

pub fn generate_p2pk_trnx(
        extended_master_private_key: &str,
        output_amount: Amount,
        rpc_info: BitcoindRpcInfo,
            ) -> Result<()> {
                
    let secp = Secp256k1::new();
    let mut rng = rand::thread_rng();
    let (_, secp256k1_pubkey) = secp.generate_keypair(&mut rng);
    let p2pk_pubkey = PublicKey::new(secp256k1_pubkey);

    let output_amount_btc = output_amount.to_btc();
    let results: (ListUnspentResultEntry, GetAddressInfoResult, Address, Amount) = get_bitcoind_info(output_amount_btc, rpc_info)?;
    let unspent_trnx = results.0;
    let input_utxo_address = results.1;
    let change_addr = results.2;
    let network_relay_fee = results.3;

    let input_utxo_derivation_path = input_utxo_address.hd_key_path.unwrap();
    let input_utxo_txid = unspent_trnx.txid.to_string();
    let input_utxo_script_pubkey = unspent_trnx.script_pub_key;
    let input_utxo_value = unspent_trnx.amount;

    let (offline, fingerprint, account_0_xpub, input_xpub) =
        ColdStorage::new(&secp, extended_master_private_key, &input_utxo_derivation_path)?;

    let online = WatchOnly::new(account_0_xpub, input_xpub, fingerprint);

    let created = online.create_psbt(&input_utxo_txid, &input_utxo_value, &output_amount, p2pk_pubkey,change_addr, network_relay_fee)?;

    let updated = online.update_psbt(created, input_utxo_script_pubkey, &input_utxo_derivation_path, &input_utxo_value)?;

    let signed = offline.sign_psbt(&secp, updated)?;

    let finalized = online.finalize_psbt(signed)?;

    // You can use `bt sendrawtransaction` to broadcast the extracted transaction.
    let tx = finalized.extract_tx_unchecked_fee_rate();

    let tx_hex = encode::serialize_hex(&tx);
    //println!("You should now be able to broadcast the following transaction: \n\n{}", hex);
    println!("{}", tx_hex);

    Ok(())
}

fn get_bitcoind_info(output_amount_btc: f64, rpc_info: BitcoindRpcInfo) -> Result<(ListUnspentResultEntry, GetAddressInfoResult, Address, Amount)> {
    
    let rpc = Client::new(&rpc_info.rpc_url,
        Auth::UserPass(rpc_info.rpc_user_id.to_string(),
        rpc_info.rpc_password.to_string())).unwrap();
    
    let network_relay_fee = rpc.get_network_info()?.relay_fee;
    let output_trnx_total = network_relay_fee.to_btc() + output_amount_btc;

    let mut unspent_option: Option<ListUnspentResultEntry> = None;
    let unspent_vec = rpc.list_unspent(Some(0), None, None, None, None).unwrap();
    for unspent_candidate in unspent_vec {
        //println!("unspent_candidate txid={}, amount={}", unspent_candidate.txid, unspent_candidate.amount.to_btc());
        if unspent_candidate.amount.to_btc()  > output_trnx_total {
            unspent_option = Some(unspent_candidate);
            break;
        }
    }
    if unspent_option == None {
        return Err(anyhow!("No unspent trnxs have sufficient funds: {}", output_trnx_total));
    }

    let unspent_trnx = unspent_option.unwrap();
    let input_utxo_address = unspent_trnx.address.clone().unwrap().assume_checked();

    let input_utxo_address_info = rpc.get_address_info(&input_utxo_address)?;

    let change_addr = rpc.get_raw_change_address(Some(json::AddressType::Bech32)).unwrap().assume_checked();

    Ok((unspent_trnx, input_utxo_address_info, change_addr, network_relay_fee))
}

// We cache the pubkeys for convenience because it requires a scep context to convert the private key.
/// An example of an offline signer i.e., a cold-storage device.
struct ColdStorage {
    /// The master extended private key.
    master_xpriv: Xpriv,
    /// The master extended public key.
    master_xpub: Xpub,
}

/// The data exported from an offline wallet to enable creation of a watch-only online wallet.
/// (wallet, fingerprint, account_0_xpub, input_utxo_xpub)
type ExportData = (ColdStorage, Fingerprint, Xpub, Xpub);

impl ColdStorage {

    /// Constructs a new `ColdStorage` signer.
    ///
    /// # Returns
    ///     The newly created signer along with the data needed to configure a watch-only wallet.
    fn new<C: Signing>(secp: &Secp256k1<C>, xpriv: &str, input_utxo_derivation_path: &DerivationPath) -> Result<ExportData> {
        let master_xpriv = Xpriv::from_str(xpriv)?;
        let master_xpub = Xpub::from_priv(secp, &master_xpriv);

        // Hardened children require secret data to derive.
        let account_0_xpriv = master_xpriv.derive_priv(secp, &input_utxo_derivation_path)?;
        let account_0_xpub = Xpub::from_priv(secp, &account_0_xpriv);

        let input_xpriv = master_xpriv.derive_priv(secp, &input_utxo_derivation_path)?;
        let input_xpub = Xpub::from_priv(secp, &input_xpriv);

        let wallet = ColdStorage { master_xpriv, master_xpub };
        let fingerprint = wallet.master_fingerprint();

        Ok((wallet, fingerprint, account_0_xpub, input_xpub))
    }

    /// Returns the fingerprint for the master extended public key.
    fn master_fingerprint(&self) -> Fingerprint { self.master_xpub.fingerprint() }

    /// Signs `psbt` with this signer.
    fn sign_psbt<C: Signing + Verification>(
        &self,
        secp: &Secp256k1<C>,
        mut psbt: Psbt,
    ) -> Result<Psbt> {
        match psbt.sign(&self.master_xpriv, secp) {
            std::result::Result::Ok(keys) => assert_eq!(keys.len(), 1),
            Err((_, e)) => {
                let e = e.get(&0).expect("at least one error");
                return Err(e.clone().into());
            }
        };
        Ok(psbt)
    }
}

/// An example of an watch-only online wallet.
struct WatchOnly {
    /// The xpub for account 0 derived from derivation path "m/84h/0h/0h".
    account_0_xpub: Xpub,
    /// The xpub derived from `INPUT_UTXO_DERIVATION_PATH`.
    input_xpub: Xpub,
    /// The master extended pubkey fingerprint.
    master_fingerprint: Fingerprint,
}

impl WatchOnly {
    /// Constructs a new watch-only wallet.
    ///
    /// A watch-only wallet would typically be online and connected to the Bitcoin network. We
    /// 'import' into the wallet the `account_0_xpub` and `master_fingerprint`.
    ///
    /// The reason for importing the `input_xpub` is so one can use bitcoind to grab a valid input
    /// to verify the workflow presented in this file.
    fn new(account_0_xpub: Xpub, input_xpub: Xpub, master_fingerprint: Fingerprint) -> Self {
        WatchOnly { account_0_xpub, input_xpub, master_fingerprint }
    }

    /// Creates the PSBT, in BIP174 parlance this is the 'Creater'.
    fn create_psbt(
            &self, 
            input_utxo_txid: &str,
            input_utxo_value: &Amount,
            output_amount_btc: &Amount,
            p2pk_pubkey: PublicKey,
            change_address: Address,
            network_relay_fee: Amount) -> Result<Psbt> {

        let output_change_total = input_utxo_value.to_btc() - network_relay_fee.to_btc() - output_amount_btc.to_btc();
        let change_amount_rounded = (output_change_total * 1000000.0).round() / 1000000.0;
/*         println!("input_utxo_value={}, network_relay_fee={}, output_amount_btc={}, change_amount_rounded={}",
            input_utxo_value.to_btc(),
            network_relay_fee.to_btc(),
            output_amount_btc.to_btc(),
            change_amount_rounded
        ); */
        let change_amount = Amount::from_float_in(change_amount_rounded, bitcoin::Denomination::Bitcoin)?;
        //let change_amount: Amount = Amount::from_str("46.99999 BTC")?; // 1000 sat transaction fee.

        let p2pk_pubkey_scriptbuf = ScriptBuf::new_p2pk(&p2pk_pubkey);

        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint { txid: input_utxo_txid.parse()?, vout: INPUT_UTXO_VOUT },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX, // Disable LockTime and RBF.
                witness: Witness::default(),
            }],
            output: vec![
                TxOut { value: *output_amount_btc, script_pubkey: p2pk_pubkey_scriptbuf },
                TxOut { value: change_amount, script_pubkey: change_address.script_pubkey() }
            ],
        };

        let psbt = Psbt::from_unsigned_tx(tx)?;

        Ok(psbt)
    }

    /// Updates the PSBT, in BIP174 parlance this is the 'Updater'.
    fn update_psbt(&self,
            mut psbt: Psbt, 
            input_utxo_script_pubkey: ScriptBuf, 
            input_utxo_derivation_path: &DerivationPath, 
            input_utxo_value: &Amount) -> Result<Psbt> {
        let t_out = TxOut { value: *input_utxo_value, script_pubkey:  input_utxo_script_pubkey};
        let mut input = Input { witness_utxo: Some(t_out), ..Default::default() };

        let pk = self.input_xpub.to_pub();
        let wpkh = pk.wpubkey_hash();

        let redeem_script = ScriptBuf::new_p2wpkh(&wpkh);
        input.redeem_script = Some(redeem_script);

        let fingerprint = self.master_fingerprint;
        let mut map = BTreeMap::new();
        map.insert(pk.0, (fingerprint, input_utxo_derivation_path.clone()));
        input.bip32_derivation = map;

        let ty = PsbtSighashType::from_str("SIGHASH_ALL")?;
        input.sighash_type = Some(ty);

        psbt.inputs = vec![input];

        Ok(psbt)
    }

    /// Finalizes the PSBT, in BIP174 parlance this is the 'Finalizer'.
    /// This is just an example. For a production-ready PSBT Finalizer, use [rust-miniscript](https://docs.rs/miniscript/latest/miniscript/psbt/trait.PsbtExt.html#tymethod.finalize)
    fn finalize_psbt(&self, mut psbt: Psbt) -> Result<Psbt> {
        if psbt.inputs.is_empty() {
            return Err(psbt::SignError::MissingInputUtxo.into());
        }

        let sigs: Vec<_> = psbt.inputs[0].partial_sigs.values().collect();
        let mut script_witness: Witness = Witness::new();
        script_witness.push(&sigs[0].to_vec());
        script_witness.push(self.input_xpub.to_pub().to_bytes());
        psbt.inputs[0].final_script_witness = Some(script_witness);

        // Clear all the data fields as per the spec.
        psbt.inputs[0].partial_sigs = BTreeMap::new();
        psbt.inputs[0].sighash_type = None;
        psbt.inputs[0].redeem_script = None;
        psbt.inputs[0].witness_script = None;
        psbt.inputs[0].bip32_derivation = BTreeMap::new();

        Ok(psbt)
    }

}

fn input_derivation_path(input_utxo_derivation_path: &str) -> Result<DerivationPath> {
    let path = input_utxo_derivation_path.into_derivation_path()?;
    Ok(path)
}

struct Error(Box<dyn std::error::Error>);

impl<T: std::error::Error + 'static> From<T> for Error {
    fn from(e: T) -> Self { Error(Box::new(e)) }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(&self.0, f) }
}
