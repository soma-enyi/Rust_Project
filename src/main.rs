// main.rs - Complete Block Explorer Backend
use actix_web::{web, App, HttpResponse, HttpServer, Responder, middleware};
use rusqlite::Connection;
use std::sync::Mutex;
use std::sync::Arc;
use serde::Serialize;

mod models;
use models::*;
mod parser;
mod db;
use db::*;
use std::path::Path;

type DbPool = Arc<Mutex<Connection>>;

// Response for latest blocks
#[derive(Serialize)]
struct LatestBlocksResponse {
    blocks: Vec<BlockSummary>,
    total_count: u32,
}

#[derive(Serialize)]
struct BlockSummary {
    hash: String,
    height: u32,
    timestamp: u32,
    tx_count: usize,
}

#[derive(Serialize)]
struct StatsResponse {
    total_blocks: u32,
    total_transactions: u64,
    latest_block_height: u32,
    latest_block_hash: String,
}

// GET /block/{hash} - Get block by hash
async fn get_block(db: web::Data<DbPool>, hash: web::Path<String>) -> impl Responder {
    let hash = hash.into_inner();
    let conn = db.lock().unwrap();
    
    match query_block(&conn, &hash) {
        Ok(Some(block)) => HttpResponse::Ok().json(block),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Block not found",
            "hash": hash
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /block/height/{height} - Get block by height
async fn get_block_by_height(db: web::Data<DbPool>, height: web::Path<u32>) -> impl Responder {
    let height = height.into_inner();
    let conn = db.lock().unwrap();
    
    match query_block_by_height(&conn, height) {
        Ok(Some(block)) => HttpResponse::Ok().json(block),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Block not found",
            "height": height
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /tx/{txid} - Get transaction by txid
async fn get_tx(db: web::Data<DbPool>, txid: web::Path<String>) -> impl Responder {
    let txid = txid.into_inner();
    let conn = db.lock().unwrap();
    
    match query_tx(&conn, &txid) {
        Ok(Some(tx)) => HttpResponse::Ok().json(tx),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Transaction not found",
            "txid": txid
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /blocks/latest?limit=10 - Get latest blocks
async fn get_latest_blocks(
    db: web::Data<DbPool>,
    query: web::Query<std::collections::HashMap<String, String>>
) -> impl Responder {
    let limit: usize = query.get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10)
        .min(100); // Max 100 blocks
    
    let conn = db.lock().unwrap();
    
    match query_latest_blocks(&conn, limit) {
        Ok(blocks) => {
            let total = query_block_count(&conn).unwrap_or(0);
            HttpResponse::Ok().json(LatestBlocksResponse {
                blocks,
                total_count: total,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /blocks?page=1&limit=20 - Get all blocks with pagination
async fn get_all_blocks(
    db: web::Data<DbPool>,
    query: web::Query<std::collections::HashMap<String, String>>
) -> impl Responder {
    let page: usize = query.get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1)
        .max(1); // Minimum page 1
    
    let limit: usize = query.get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(20)
        .min(100); // Max 100 blocks per page
    
    let offset = (page - 1) * limit;
    
    let conn = db.lock().unwrap();
    
    match query_all_blocks(&conn, limit, offset) {
        Ok(blocks) => {
            let total = query_block_count(&conn).unwrap_or(0);
            let total_pages = (total as f64 / limit as f64).ceil() as usize;
            
            HttpResponse::Ok().json(serde_json::json!({
                "blocks": blocks,
                "pagination": {
                    "current_page": page,
                    "per_page": limit,
                    "total_blocks": total,
                    "total_pages": total_pages,
                    "has_next": page < total_pages,
                    "has_prev": page > 1
                }
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /block/{hash}/transactions - Get all transactions in a block
async fn get_block_transactions(
    db: web::Data<DbPool>,
    hash: web::Path<String>
) -> impl Responder {
    let hash = hash.into_inner();
    let conn = db.lock().unwrap();
    
    match query_block_transactions(&conn, &hash) {
        Ok(txs) => HttpResponse::Ok().json(serde_json::json!({
            "block_hash": hash,
            "transaction_count": txs.len(),
            "transactions": txs
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "message": e.to_string()
        })),
    }
}

// GET /stats - Get blockchain statistics
async fn get_stats(db: web::Data<DbPool>) -> impl Responder {
    let conn = db.lock().unwrap();
    
    let total_blocks = query_block_count(&conn).unwrap_or(0);
    let total_txs = query_transaction_count(&conn).unwrap_or(0);
    
    let latest = query_latest_block(&conn);
    
    match latest {
        Ok(Some((height, hash))) => {
            HttpResponse::Ok().json(StatsResponse {
                total_blocks,
                total_transactions: total_txs,
                latest_block_height: height,
                latest_block_hash: hash,
            })
        }
        _ => HttpResponse::Ok().json(serde_json::json!({
            "total_blocks": total_blocks,
            "total_transactions": total_txs,
            "message": "No blocks indexed yet"
        })),
    }
}

// GET /health - Health check endpoint
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "block-explorer-backend"
    }))
}

// Query functions
fn query_block(conn: &Connection, hash: &str) -> rusqlite::Result<Option<BlockResponse>> {
    let mut stmt = conn.prepare(
        "SELECT hash, height, version, prev_block, merkle_root, timestamp, bits, nonce, size 
         FROM blocks WHERE hash = ?"
    )?;
    
    let mut rows = stmt.query_map([hash], |row| {
        let tx_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE block_hash = ?",
            [hash],
            |r| r.get(0)
        ).unwrap_or(0);

        Ok(BlockResponse {
            hash: row.get(0)?,
            height: row.get(1)?,
            version: row.get(2)?,
            prev_block: row.get(3)?,
            merkle_root: row.get(4)?,
            timestamp: row.get(5)?,
            bits: row.get(6)?,
            nonce: row.get(7)?,
            size: row.get(8)?,
            tx_count,
        })
    })?;
    
    rows.next().transpose()
}

fn query_block_by_height(conn: &Connection, height: u32) -> rusqlite::Result<Option<BlockResponse>> {
    let hash: String = match conn.query_row(
        "SELECT hash FROM blocks WHERE height = ?",
        [height],
        |row| row.get(0)
    ) {
        Ok(h) => h,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e),
    };
    
    query_block(conn, &hash)
}

fn query_tx(conn: &Connection, txid: &str) -> rusqlite::Result<Option<TxResponse>> {
    let mut stmt = conn.prepare(
        "SELECT txid, block_hash, inputs, outputs, raw_data FROM transactions WHERE txid = ?"
    )?;
    
    let mut rows = stmt.query_map([txid], |row| {
        let inputs: String = row.get(2)?;
        let outputs: String = row.get(3)?;
        let raw_data: Vec<u8> = row.get(4)?;
        let block_hash: Option<String> = row.get(1)?;
        
        let block_height = if let Some(ref bh) = block_hash {
            conn.query_row(
                "SELECT height FROM blocks WHERE hash = ?",
                [bh],
                |r| r.get(0)
            ).ok()
        } else {
            None
        };
        
        Ok(TxResponse {
            txid: row.get(0)?,
            version: 1,
            lock_time: 0,
            block_hash,
            block_height,
            confirmations: None,
            inputs: serde_json::from_str(&inputs).unwrap_or_default(),
            outputs: serde_json::from_str(&outputs).unwrap_or_default(),
            size: raw_data.len(),
            vsize: raw_data.len(),
            weight: raw_data.len() * 4,
        })
    })?;
    
    rows.next().transpose()
}

fn query_latest_blocks(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<BlockSummary>> {
    let mut stmt = conn.prepare(
        "SELECT hash, height, timestamp FROM blocks ORDER BY height DESC LIMIT ?"
    )?;
    
    let blocks = stmt.query_map([limit], |row| {
        let hash: String = row.get(0)?;
        let tx_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE block_hash = ?",
            [&hash],
            |r| r.get(0)
        ).unwrap_or(0);
        
        Ok(BlockSummary {
            hash,
            height: row.get(1)?,
            timestamp: row.get(2)?,
            tx_count,
        })
    })?;
    
    blocks.collect()
}

fn query_all_blocks(conn: &Connection, limit: usize, offset: usize) -> rusqlite::Result<Vec<BlockSummary>> {
    let mut stmt = conn.prepare(
        "SELECT hash, height, timestamp FROM blocks ORDER BY height ASC LIMIT ? OFFSET ?"
    )?;
    
    let blocks = stmt.query_map([limit, offset], |row| {
        let hash: String = row.get(0)?;
        let tx_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE block_hash = ?",
            [&hash],
            |r| r.get(0)
        ).unwrap_or(0);
        
        Ok(BlockSummary {
            hash,
            height: row.get(1)?,
            timestamp: row.get(2)?,
            tx_count,
        })
    })?;
    
    blocks.collect()
}

fn query_block_transactions(conn: &Connection, block_hash: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT txid FROM transactions WHERE block_hash = ?"
    )?;
    
    let txids = stmt.query_map([block_hash], |row| row.get(0))?;
    txids.collect()
}

fn query_block_count(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))
}

fn query_transaction_count(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
}

fn query_latest_block(conn: &Connection) -> rusqlite::Result<Option<(u32, String)>> {
    match conn.query_row(
        "SELECT height, hash FROM blocks ORDER BY height DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?))
    ) {
        Ok(result) => Ok(Some(result)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Logging disabled - add env_logger dependency to enable
    // cargo add env_logger log
    
    let db_path = Path::new("blocks.db");
    println!("Initializing database at {:?}", db_path);
    
    let conn = init_db(db_path).expect("Failed to initialize database");
    let db_pool: DbPool = Arc::new(Mutex::new(conn));

    // Optional: Index blocks on startup
    // Uncomment the section below to index Bitcoin blocks
    
    println!("Starting block indexing...");
    {
        let conn = db_pool.lock().unwrap();
        // Update this path to your Bitcoin blocks directory
        let blocks_path = Path::new("/home/mesoma/.bitcoin/regtest/blocks");
        
        match parser::index_blocks(&conn, blocks_path).await {
            Ok(_) => println!("Block indexing complete!"),
            Err(e) => {
                eprintln!("Warning: Block indexing failed: {}", e);
                eprintln!("Continuing without indexed blocks...");
            }
        }
    }
    

    let bind_address = "127.0.0.1:8080";
    println!("Starting server at http://{}", bind_address);
    println!("\nAvailable endpoints:");
    println!("  GET /health");
    println!("  GET /stats");
    println!("  GET /blocks?page=1&limit=20");
    println!("  GET /blocks/latest?limit=10");
    println!("  GET /block/{{hash}}");
    println!("  GET /block/height/{{height}}");
    println!("  GET /block/{{hash}}/transactions");
    println!("  GET /tx/{{txid}}");

    HttpServer::new(move || {
        App::new()
            // .wrap(middleware::Logger::default())  // Uncomment when env_logger is added
            .wrap(middleware::Compress::default())
            .app_data(web::Data::new(db_pool.clone()))
            .route("/health", web::get().to(health_check))
            .route("/stats", web::get().to(get_stats))
            .route("/blocks", web::get().to(get_all_blocks))
            .route("/blocks/latest", web::get().to(get_latest_blocks))
            .route("/block/{hash}", web::get().to(get_block))
            .route("/block/height/{height}", web::get().to(get_block_by_height))
            .route("/block/{hash}/transactions", web::get().to(get_block_transactions))
            .route("/tx/{txid}", web::get().to(get_tx))
    })
    .bind(bind_address)?
    .run()
    .await
}