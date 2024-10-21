gabriel

- [1. Introduction](#1-introduction)
- [2. Setup](#2-setup)
  - [2.1. Pre-reqs](#21-pre-reqs)
    - [2.1.1. Hardware](#211-hardware)
    - [2.1.2. Software](#212-software)
      - [2.1.2.1. Rust](#2121-rust)
      - [2.1.2.2. bitcoind](#2122-bitcoind)
  - [2.2. Clone code](#22-clone-code)
  - [2.3. Build](#23-build)
  - [2.4. Execute tests](#24-execute-tests)
- [3. Run Gabriel](#3-run-gabriel)
  - [3.1. analyze single block data file](#31-analyze-single-block-data-file)
  - [3.2. analyze all block data files](#32-analyze-all-block-data-files)
  - [3.3. consume and analyze new raw blocks](#33-consume-and-analyze-new-raw-blocks)
    - [3.3.1. Fund a trnx w/ a P2PK output on reg-test](#331-fund-a-trnx-w-a-p2pk-output-on-reg-test)
    - [3.3.2. Generate block:](#332-generate-block)
    - [3.3.3. Test](#333-test)
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

### 2.2. Clone code

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

### 3.1. analyze single block data file

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ export BITCOIND_BLOCK_DATA_FILE=xxx.dat

        $ ./target/debug/gabriel block-file-eval \
            -b $BITCOIND_DATA_DIR/blocks/$BITCOIND_BLOCK_DATA_FILE \
            -o /tmp/$BITCOIND_BLOCK_DATA_FILE.csv


### 3.2. analyze all block data files

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ ./target/debug/gabriel index \
            --input $BITCOIND_DATA_DIR/blocks \
            --output /tmp/gabriel-testnet4.csv

### 3.3. consume and analyze new raw blocks

        $ ./target/debug/gabriel block-async-eval \
            --zmqpubrawblock-socket-url tcp://127.0.0.1:29001 \
            --output /tmp/async_blocks.txt

#### 3.3.1. Fund a trnx w/ a P2PK output on reg-test

1. Get extended private key:

        $ export W_NAME=lightning && export WPASS=lightning
        $ b-reg -rpcwallet=$W_NAME walletpassphrase $WPASS 120
        $ XPRV=$( b-reg gethdkeys '{"active_only":true, "private":true}' | jq -r .[].xprv ) && echo $XPRV

2. Create a trnx w/ P2PK output:
   
        $ export RUST_BACKTRACE=1
        $ SIGNED_P2PK_RAW_TRNX=$( ./target/debug/gabriel generate-p2pk-trnx )

3.  Send trnx:

        $ b-reg sendrawtransaction $SIGNED_P2PK_RAW_TRNX

#### 3.3.2. Generate block:
    
        $ b-reg -generate 1
   


#### 3.3.3. Test

TO-DO:  generate a test P2PK address and send block rewards

## 4. Debug in VSCode:

Add and edit the following to $PROJECT_HOME/.vscode/launch.json:

`````
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "gabriel local: 'block-file-eval'",
            "args": ["block-file-eval", "-b=/u04/bitcoin/datadir/blocks/blk00000.dat", "-o=/tmp/blk00000.dat.csv"],
            "cwd": "${workspaceFolder}",
            "program": "./target/debug/gabriel",
            "sourceLanguages": ["rust"]
        }
    ]
}
`````

