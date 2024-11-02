gabriel

- [1. Introduction](#1-introduction)
- [2. Setup](#2-setup)
  - [2.1. Pre-reqs](#21-pre-reqs)
    - [2.1.1. Hardware](#211-hardware)
    - [2.1.2. Software](#212-software)
      - [2.1.2.1. Rust](#2121-rust)
      - [2.1.2.2. bitcoind](#2122-bitcoind)
      - [2.1.2.3. SQLite client](#2123-sqlite-client)
  - [2.2. Clone Gabriel](#22-clone-gabriel)
  - [2.3. Build](#23-build)
  - [2.4. Execute tests](#24-execute-tests)
- [3. Run Gabriel](#3-run-gabriel)
  - [3.1. environment variables](#31-environment-variables)
  - [3.2. analyze all block data files](#32-analyze-all-block-data-files)
  - [3.3. analyze single block data file](#33-analyze-single-block-data-file)
    - [3.3.1. Optional:  debug via VSCode:](#331-optional--debug-via-vscode)
  - [3.4. monitor new raw block events and run web app](#34-monitor-new-raw-block-events-and-run-web-app)
    - [3.4.1. Fund a tx w/ a P2PK output on reg-test](#341-fund-a-tx-w-a-p2pk-output-on-reg-test)
    - [3.4.2. Generate block:](#342-generate-block)
    - [3.4.3. Optional: dDebug in VSCode:](#343-optional-ddebug-in-vscode)
    - [View P2PK charts](#view-p2pk-charts)
- [4. Inspect P2PK Analysis data](#4-inspect-p2pk-analysis-data)


## 1. Introduction
Measures how many unspent public key addresses there are, and how many coins are in them over time. Early Satoshi-era coins that are just sitting with exposed public keys. If we see lots of coins move... That's a potential sign that quantum computers have silently broken bitcoin.

## 2. Setup

### 2.1. Pre-reqs

#### 2.1.1. Hardware

Gabriel requires a fully synced Bitcoin Core daemon to be running.
Your hardware requirements will vary depending on the bitcoin network(ie: main, testnet4, regtest, etc) you choose.

If running in _regtest_ (ie: for dev / test purposes) then use of a modern laptop will be plenty sufficient.

#### 2.1.2. Software
##### 2.1.2.1. Rust
The best way to install Rust is to use [rustup](https://rustup.rs).

##### 2.1.2.2. bitcoind

Gabriel requires a fully synced Bitcoin Core daemon to be running.
For testing and development purposes, running Bitcoin Core on _regtest_ is sufficient.

If on bitcoind v28.0, ensure the following flag is set prior to initial block download:  `-blocksxor=0`

1. Start Bitcoin Core:
   The following example starts Bitcoin Core in _regtest_ mode.


        $ bitcoind \
          -regtest \
          -server -daemon \
          -fallbackfee=0.0002 \
          -rpcuser=admin -rpcpassword=pass \
          -rpcallowip=127.0.0.1/0 -rpcbind=127.0.0.1 \
          -blockfilterindex=1 -peerblockfilters=1 \
          -zmqpubrawblock=unix:/tmp/zmqpubrawblock.unix \
          -blocksxor=0

   NOTE: Gabriel includes functionality that consumes block events from Bitcoin Core via its _zmqpubrawblock_ ZeroMQ interface.
   The example above specifies a Unix domain socket.
   Alternatively, you could choose to specify a tcp socket and port similar to the following:  `-zmqpubrawblock=tcp://127.0.0.1:29001`

2. Define a shell alias to `bitcoin-cli`, for example:
   
                $ `alias b-reg=bitcoin-cli -rpcuser=admin -rpcpassword=pass -rpcport=18443`

3. Create (or load) a default wallet, for example:

                $ `b-reg createwallet <wallet-name>`

4. Mine some blocks, for example:

                $ `b-reg generatetoaddress 110 $(b-reg getnewaddress)`


##### 2.1.2.3. SQLite client

Gabriel persists P2PK utxo analysis to a local SQLite database.
You will need to download and install the  [SQLite client](https://sqlite.org/download.html) for your operating system.

Once installed, set the SQLITE_ABSOLUTE_PATH environment variable to the path of the SQLite database:

        $ export SQLITE_ABSOLUTE_PATH=/path/to/gabriel_p2pk.db
      
### 2.2. Clone Gabriel

You'll need the Gabriel source code:

```
$ git clone https://github.com/SurmountSystems/gabriel.git
$ git checkout HB/gabriel-v2
```

### 2.3. Build

* execute:

        $ cargo build

* view Gabriel's command line options:


        $ ./target/debug/gabriel

### 2.4. Execute tests

```
$ cargo test
```

## 3. Run Gabriel

### 3.1. environment variables

For all operations, Gabriel uses the following environment variables:


- BITCOIND_RPC_URL
- BITCOIND_RPC_COOKIE_PATH
  - or
    - BITCOIND_RPC_USER
    - BITCOIND_RPC_PASS
- SQLITE_ABSOLUTE_PATH

Optional:
- RUST_LOG
  - set to a valid value (ie: "info", "debug", "error", etc) to override default logging level
- RUST_BACKTRACE
  - set to 1 to enable backtrace

### 3.2. analyze all block data files

Gabriel can be used to identify P2PK utxos across all transactions.

Execute the following if analyzing the entire (previously downloaded) Bitcoin blockchain:

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ ./target/debug/gabriel index \
            --input $BITCOIND_DATA_DIR/blocks \
            --output /tmp/gabriel-testnet4.csv

### 3.3. analyze single block data file

Alternatively, you can have (likely for testing purposes) Gabriel analyze a single Bitcoin Core block data file.

Execute as follows:

        $ export BITCOIND_DATA_DIR=/path/to/bitcoind/data/dir
        $ export BITCOIND_BLOCK_DATA_FILE=xxx.dat
        $ export SQLITE_ABSOLUTE_PATH=/tmp/gabriel_p2pk.db

        $ ./target/debug/gabriel single-block-file-eval \
            -b $BITCOIND_DATA_DIR/blocks/$BITCOIND_BLOCK_DATA_FILE

#### 3.3.1. Optional:  debug via VSCode:

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

### 3.4. monitor new raw block events and run web app

After identifying P2PK utxos from an Initial Block Download (IBD), Gabriel can asynchronously consume new block events as generated by your Bitcoin Core node.

Execute as follows:

1) Set needed environment variables:
   ```
   $ export ZMQ_SOCKET_URL=ipc:///tmp/zmqpubrawblock.unix
   $ export SQLITE_ABSOLUTE_PATH=/tmp/gabriel_p2pk.db
   ```

   NOTE: The above configures Gabriel to consume block events using the same ZeroMQ Unix domain socket that Bitcoin Core was previously configured to produce to.
   If your Bitcoin Core daemon is configured to use TCP for its ZeroMQ interfaces, then you will want Gabriel to instead use a TCP consumer:

   ```
   $ export ZMQ_SOCKET_URL=tcp://127.0.0.1:29001
   ```

2) Execute:
```
$ ./target/debug/gabriel block-async-eval-and-web-app
```

#### 3.4.1. Fund a tx w/ a P2PK output on reg-test

If interested in testing Gabriel's ability to consume and process a block with a P2PK utxo, you can use the following in a new terminal:

1. Get extended private key from bitcoind:\
   
   NOTE:  for the following command, you'll already need to have unlocked your wallet via the bitcoin cli.

        $ XPRV=$( b-reg gethdkeys '{"active_only":true, "private":true}' \
            | jq -r .[].xprv ) && echo $XPRV

2. Create a tx w/ P2PK output:
   
        $ export BITCOIND_RPC_URL=http://127.0.0.1:18443 \
            && export BITCOIND_RPC_COOKIE_PATH=/path/to/bitcoind/datadir/regtest/.cookie
   
        $ SIGNED_P2PK_RAW_TX=$( ./target/debug/gabriel \
                generate-p2pk-tx \
                -e $XPRV ) \
                && echo $SIGNED_P2PK_RAW_TX

3. View decoded tx:
   
        $ b-reg decoderawtransaction $SIGNED_P2PK_RAW_TX

4.  Send tx:

        $ b-reg sendrawtransaction $SIGNED_P2PK_RAW_TX

#### 3.4.2. Generate block:
    
        $ b-reg -generate 1
   
        NOTE:  You should now see a new record in Gabriel's output file indicating the new P2PK utxo.


#### 3.4.3. Optional: dDebug in VSCode:

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

#### View P2PK charts

## 4. Inspect P2PK Analysis data
Gabriel will persist analysis of P2PK utxos in a SQLite database.

The path of the SQLite database is the value of the SQLITE_ABSOLUTE_PATH environment variable.

At the command line, you can inspect the data in SQLite database similar to the following:

```
$ sqlite3 $SQLITE_ABSOLUTE_PATH

# list tables;
sqlite> .tables

# view the schema of the   p2pk_utxo_block_aggregates table:
sqlite> .schema p2pk_utxo_block_aggregates

# identify number of records in p2pk_utxo_block_aggregates table
sqlite> select count(block_height) from p2pk_utxo_block_aggregates;

# delete all records
sqlite> delete from p2pk_utxo_block_aggregates;

# quit sqlite command line:  press  <ctrl> d

```
