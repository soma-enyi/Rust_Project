use rusqlite::{Connection, Result};
use std::path::Path;
use crate::models::{TxInSimplified, TxOutSimplified};

// Initialize DB and create tables
pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocks (
            hash TEXT PRIMARY KEY,
            height INTEGER,
            version INTEGER,
            prev_block TEXT,
            merkle_root TEXT,
            timestamp INTEGER,
            bits INTEGER,
            nonce INTEGER,
            size INTEGER,
            header BLOB,
            raw_data BLOB
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            txid TEXT PRIMARY KEY,
            block_hash TEXT,
            inputs TEXT,
            outputs TEXT,
            raw_data BLOB,
            FOREIGN KEY (block_hash) REFERENCES blocks(hash)
        )",
        [],
    )?;
    Ok(conn)
}

// Insert a test mainnet genesis block for testing
pub fn insert_test_block(conn: &Connection) -> Result<()> {
    let genesis_hex = "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c0101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff4d04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73ffffffff0100f2052a01000000434104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac00000000";
    let genesis_bytes = hex::decode(genesis_hex).unwrap();
    let genesis: bitcoin::Block = bitcoin::consensus::deserialize(&genesis_bytes).unwrap();
    insert_block(conn, &genesis, 0)
}


// Function to insert a block
pub fn insert_block(conn: &Connection, block: &bitcoin::Block, height: u32) -> Result<()> {
    let hash = block.block_hash().to_string();
    let header = &block.header;
    let header_blob = bitcoin::consensus::encode::serialize(header);
    let raw_data = bitcoin::consensus::encode::serialize(block);

    conn.execute(
        "INSERT OR REPLACE INTO blocks (hash, height, version, prev_block, merkle_root, timestamp, bits, nonce, size, header, raw_data) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            &hash,
            &height,
            &header.version.to_consensus(),
            &header.prev_blockhash.to_string(),
            &header.merkle_root.to_string(),
            &header.time,
            &header.bits.to_consensus(),
            &header.nonce,
            &raw_data.len(),
            &header_blob,
            &raw_data
        ],
    )?;

    for tx in &block.txdata {
        insert_tx(conn, tx, &hash)?;
    }
    Ok(())
}

// Function to insert a transaction
// FIXED: Convert TxIn/TxOut to serializable versions
pub fn insert_tx(conn: &Connection, tx: &bitcoin::Transaction, block_hash: &str) -> Result<()> {
    let txid = tx.compute_txid().to_string();
    
    // Convert inputs to simplified version
    let inputs: Vec<TxInSimplified> = tx.input.iter().map(|input| {
        TxInSimplified {
            prev_txid: input.previous_output.txid.to_string(),
            vout: input.previous_output.vout,
            script_sig: hex::encode(&input.script_sig.as_bytes()),
            sequence: input.sequence.0,
            witness: input.witness.iter()
                .map(|w| hex::encode(w))
                .collect(),
        }
    }).collect();
    
    // Convert outputs to simplified version
    let outputs: Vec<TxOutSimplified> = tx.output.iter().map(|output| {
        TxOutSimplified {
            value: output.value.to_sat(),
            script_pubkey: hex::encode(&output.script_pubkey.as_bytes()),
        }
    }).collect();
    
    let inputs_json = serde_json::to_string(&inputs).unwrap();
    let outputs_json = serde_json::to_string(&outputs).unwrap();
    let raw_data = bitcoin::consensus::encode::serialize(tx);

    conn.execute(
        "INSERT OR REPLACE INTO transactions (txid, block_hash, inputs, outputs, raw_data) 
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![&txid, block_hash, &inputs_json, &outputs_json, &raw_data],
    )?;
    Ok(())
}