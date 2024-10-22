gabriel

- [1. Introduction](#1-introduction)
- [2. Setup](#2-setup)
  - [2.1. Pre-reqs](#21-pre-reqs)
    - [2.1.1. Hardware](#211-hardware)
    - [2.1.2. Software](#212-software)
      - [2.1.2.1. Rust](#2121-rust)
      - [2.1.2.2. bitcoind](#2122-bitcoind)
  - [2.2. Clone](#22-clone)
  - [2.3. Build](#23-build)
  - [2.4. Execute tests](#24-execute-tests)
- [3. Run Gabriel](#3-run-gabriel)
  - [3.2. analyze all block data files](#32-analyze-all-block-data-files)
  - [3.1. analyze single block data file](#31-analyze-single-block-data-file)
    - [Optional:  debug via VSCode:](#optional--debug-via-vscode)
  - [3.3. consume and analyze new raw block events](#33-consume-and-analyze-new-raw-block-events)
    - [3.3.1. Fund a tx w/ a P2PK output on reg-test](#331-fund-a-tx-w-a-p2pk-output-on-reg-test)
    - [3.3.2. Generate block:](#332-generate-block)
- [4. Debug in VSCode:](#4-debug-in-vscode)


## 1. Introduction
Measures how many unspent public key addresses there are, and how many coins are in them over time. Early Satoshi-era coins that are just sitting with exposed public keys. If we see lots of coins move... That's a potential sign that quantum computers have silently broken bitcoin.

## 2. Setup

### 2.1. Pre-reqs
```
$ bitcoind \
    -conf=$GITEA_HOME/blockchain/bitcoin/admin/bitcoind/bitcoin.conf \
    -daemon=0
```

#### 2.1.1. Hardware

#### 2.1.2. Software
##### 2.1.2.1. Rust
The best way to install Rust is to use [rustup](https://rustup.rs).

##### 2.1.2.2. bitcoind

If on bitcoind v28.0, ensure the following flag is set prior to initial block download:  `-blocksxor=0`

1. Start Bitcoin Core in Regtest mode, for example:


                $ bitcoind \
                        -regtest \
                        -server -daemon \
                        -fallbackfee=0.0002 \
                        -rpcuser=admin -rpcpassword=pass -rpcallowip=127.0.0.1/0 -rpcbind=127.0.0.1 \
                        -blockfilterindex=1 -peerblockfilters=1 \
                        -blocksxor=0

2. Define a shell alias to `bitcoin-cli`, for example:
   
                $ `alias b-reg=bitcoin-cli -rpcuser=admin -rpcpassword=pass -rpcport=18443`

3. Create (or load) a default wallet, for example:

                $ `b-reg createwallet <wallet-name>`

4. Mine some blocks, for example:

                $ `b-reg generatetoaddress 110 $(b-reg getnewaddress)`

### 2.2. Clone

```
$ git clone https://github.com/SurmountSystems/gabriel.git
$ git checkout HB/gabriel-v2
```

### 2.3. Build

* execute:

        $ cargo build

* view gabriel command line options:


        $ ./target/debug/gabriel

### 2.4. Execute tests

```
$ cargo test
```

## 3. Run Gabriel

### 3.2. analyze all block data files

Execute the following if analyzing the entire (previously downloaded) Bitcoin blockchain:

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ ./target/debug/gabriel index \
            --input $BITCOIND_DATA_DIR/blocks \
            --output /tmp/gabriel-testnet4.csv

### 3.1. analyze single block data file

Alternatively, you can have (likely for testing purposes) Gabriel analyze a single Bitcoin Core block data file.
Execute as follows:

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ export BITCOIND_BLOCK_DATA_FILE=xxx.dat

        $ ./target/debug/gabriel block-file-eval \
            -b $BITCOIND_DATA_DIR/blocks/$BITCOIND_BLOCK_DATA_FILE \
            -o /tmp/$BITCOIND_BLOCK_DATA_FILE.csv

#### Optional:  debug via VSCode:

Modify the following as appropriate and add to your vscode `launch.json`:
        
        {
          "version": "0.2.0",
          "configurations": [
            {
                "type": "lldb",
                "request": "launch",
                "name": "gabriel local: 'block-file-eval'",
                "args": ["block-file-eval", "-b=/tmp/<changeme>.dat", "-o=/tmp/<changeme>.dat.csv"],
                "cwd": "${workspaceFolder}",
                "program": "./target/debug/gabriel",
                "sourceLanguages": ["rust"]
            }
          ]
        }

### 3.3. consume and analyze new raw block events

        $ ./target/debug/gabriel block-async-eval \
            --zmqpubrawblock-socket-url tcp://127.0.0.1:29001 \
            --output /tmp/async_blocks.txt

#### 3.3.1. Fund a tx w/ a P2PK output on reg-test

If interested in testing Gabriel's ability to consume and process a block with a P2PK utxo, you can use the following in a new terminal:

1. Get extended private key from bitcoind:\
   
   NOTE:  for the following command, you'll already need to have unlocked your wallet via the bitcoin cli.

        $ XPRV=$( b-reg gethdkeys '{"active_only":true, "private":true}' \
        | jq -r .[].xprv ) && echo $XPRV

2. Create a tx w/ P2PK output:
   
        $ SIGNED_P2PK_RAW_TX=$( ./target/debug/gabriel \
                generate-p2pk-tx \
                -e $XPRV ) \
                && echo $SIGNED_P2PK_RAW_TX

3. View decoded tx:
   
        $ b-reg decoderawtransaction $SIGNED_P2PK_RAW_TX

4.  Send tx:

        $ b-reg sendrawtransaction $SIGNED_P2PK_RAW_TX

#### 3.3.2. Generate block:
    
        $ b-reg -generate 1
   
        NOTE:  You should now see a new record in Gabriel's output file indicating the new P2PK utxo.


## 4. Debug in VSCode:

Add and edit the following to $PROJECT_HOME/.vscode/launch.json:

        {
          "version": "0.2.0",
          "configurations": [
            {
                "type": "lldb",
                "request": "launch",
                "name": "gabriel local: 'generate-p2pk-trnx'",
                "args": ["generate-p2pk-trnx", "-e=$XPRV-CHANGEME"],
                "cwd": "${workspaceFolder}",
                "program": "./target/debug/gabriel",
                "sourceLanguages": ["rust"]
            }
          ]
        }

