use std::env;

use light_client_minimal::{net::rpc::RpcClient, store::file::FileStore, sync::sync_chain};
use tracing_subscriber::EnvFilter;
use figlet_rs::FIGfont;
use colored::*;
use clap::Parser;

fn print_banner() {
    // Load a custom font from file, or fall back to standard font
    let font = if let Ok(custom_font) = FIGfont::from_file("fonts/cyberpunk.flf") {
        custom_font
    } else if let Ok(custom_font) = FIGfont::from_file("crates/light_client_minimal/fonts/cyberpunk.flf") {
        custom_font
    } else {
        // Fall back to standard font if custom font not found
        FIGfont::standard().unwrap()
    };

    let figure = font.convert("Zoro Zero").unwrap();
    
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════".bright_magenta());
    println!("{}", figure.to_string().bright_cyan().bold());
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════".bright_magenta());
    println!("{}", "ZK Client for Zcash • Written in Cairo Zero".bright_yellow());
    println!("{}", "Inspired by Bankai".truecolor(255, 165, 0));
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════".bright_magenta());
    println!();
}

#[derive(Parser, Debug)]
#[command(name = "zoro-zero")]
#[command(about = "ZK Client for Zcash • Written in Cairo Zero", long_about = None)]
struct Args {
    /// Generate STWO proofs for each verified block
    #[arg(short, long)]
    prove: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_banner();
    
    let args = Args::parse();
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("stwo=warn".parse().unwrap())
        .add_directive("stwo_prover=warn".parse().unwrap())
        .add_directive("stwo_cairo_prover=warn".parse().unwrap())
        .add_directive("stwo_cairo_adapter=warn".parse().unwrap())
        .add_directive("stwo_cairo_utils=warn".parse().unwrap())
        .add_directive("stwo_cairo_serialize=warn".parse().unwrap())
        .add_directive("cairo_air=warn".parse().unwrap())
        .add_directive("run=warn".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let url = env::var("ZCASH_RPC_URL").expect("ZCASH_RPC_URL must be set");
    let client = RpcClient::new(&url)?;

    let start_height: u32 = match env::var("START_HEIGHT") {
        Ok(s) => s.parse().expect("START_HEIGHT must be a valid u32"),
        Err(_) => 3_000_000,
    };

    let store = FileStore::new("./data/headers.jsonl")?;
    sync_chain(&client, &store, start_height, args.prove).await?;

    Ok(())
}
