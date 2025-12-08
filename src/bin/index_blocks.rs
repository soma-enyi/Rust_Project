use block_explorer_backend::db;
use block_explorer_backend::parser;
use std::path::Path;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let blocks_dir = env::args()
        .nth(1)
        .unwrap_or_else(|| {
            let home = env::var("HOME").expect("HOME not set");
            format!("{}/.bitcoin/regtest/blocks", home)
        });
    
    let db_path = Path::new("blocks.db");
    let blocks_path = Path::new(&blocks_dir);
    
    println!("Block Explorer Indexer");
    println!("Database: {:?}", db_path);
    println!("Blocks directory: {:?}", blocks_path);
    
    if !blocks_path.exists() {
        eprintln!("Error: Blocks directory does not exist");
        std::process::exit(1);
    }
    
    println!("Initializing database...");
    let conn = db::init_db(db_path)?;
    // Insert a test block for demonstration
    match db::insert_test_block(&conn) {
        Ok(_) => println!("Inserted test genesis block"),
        Err(e) => eprintln!("Failed to insert test block: {}", e),
    }
    println!("Starting block indexing...");
    
    match parser::index_blocks(&conn, blocks_path).await {
        Ok(_) => {
            let block_count: u32 = conn.query_row(
                "SELECT COUNT(*) FROM blocks", [], |row| row.get(0)
            )?;
            let tx_count: u64 = conn.query_row(
                "SELECT COUNT(*) FROM transactions", [], |row| row.get(0)
            )?;
            
            println!("Indexing complete!");
            println!("Blocks: {}", block_count);
            println!("Transactions: {}", tx_count);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}
