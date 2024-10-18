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
    - [3.3.1. Fund a P2PK address on reg-test](#331-fund-a-p2pk-address-on-reg-test)
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

#### 3.3.1. Fund a P2PK address on reg-test

NOTE:  FOr this exercise, start w/ a fresh reg-test environment (with no blocks yet generated)

1. On reg-test, create a new address and corresponding public key:

        $ TARGET_ADDR=$( b-reg getnewaddress ) \
            && echo $TARGET_ADDR \
            && TARGET_PUB_KEY=$( b-reg getaddressinfo $TARGET_ADDR | jq -r .pubkey ) \
            && echo $TARGET_PUB_KEY

2. Create an initial raw trnx:
   
        $ INITIAL_RAW_TRNX=$( b-reg createrawtransaction "[]" "[{\"$TARGET_ADDR\":49.99971800}]" 0 true ) \
            && echo $INITIAL_RAW_TRNX

3. Fund initial trnx:
   
        $ FUNDED_RAW_TRNX=$( b-reg fundrawtransaction $INITIAL_RAW_TRNX '{"subtractFeeFromOutputs":[0],"fee_rate":200}' \
            | jq -r .hex ) \
            && echo $FUNDED_RAW_TRNX

4. View decoded trnx:

        $ b-reg decoderawtransaction $FUNDED_RAW_TRNX

5. Generate ScriptPubKey from your public key:

        $ P2PK_SCRIPT_PUB_KEY=$( ./target/debug/gabriel \
            generate-script-pub-key-from-pub-key --pub-key=$TARGET_PUB_KEY ) \
            && b-reg decodescript $P2PK_SCRIPT_PUB_KEY

6. TO-DO:  TOTAL HACK :  Swap output of funded trnx with P2PK:

        // https://learnmeabitcoin.com/technical/transaction/output/#scriptpubkey-size
        // https://bitcointalk.org/index.php?topic=5465605.msg62794648#msg62794648

        020000000138597989d9eb741c551d3c5949ef47330dbfbad99e85ee1e4aad4a5bf752a5a80100000000fdffffff012531000000000000

        160014203e1c96fc3083329aaa12e4deafdcd621ffc856 //this part should be replaced
        00000000

        P2PK_RAW_TRNX=020000000116206f68ec8b12b3c1d4b13e045cb3750191d490f7932e814ba29bf1d38177de0000000000fdffffff01109c052a010000002321033fac86cc916b4750c434641e86f08c50a43f3f83d0f1869ec51403833f57ae43ac00000000

7. View funded trnx w/ P2PK output:

        $ b-reg decoderawtransaction $P2PK_RAW_TRNX

8. Sign trnx:

        $ SIGNED_P2PK_RAW_TRNX=$( b-reg signrawtransactionwithwallet $P2PK_RAW_TRNX \
            | jq -r .hex ) \
            && echo $SIGNED_P2PK_RAW_TRNX

9.  Send trnx:

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
            "name": "Debug gabriel local: 'block-file-eval'",
            "args": ["block-file-eval", "-b=/u04/bitcoin/datadir/blocks/blk00000.dat", "-o=/tmp/blk00000.dat.csv"],
            "cwd": "${workspaceFolder}",
            "program": "./target/debug/gabriel",
            "sourceLanguages": ["rust"]
        }
    ]
}
`````

