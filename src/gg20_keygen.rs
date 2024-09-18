use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use log::{debug, info, warn};
use std::path::PathBuf;
use structopt::StructOpt;

use curv::elliptic::curves::secp256_k1::Secp256k1;
use multi_party_ecdsa::protocols::multi_party_ecdsa::gg_2020::state_machine::keygen::{
    Keygen, LocalKey,
};
use round_based::async_runtime::AsyncProtocol;

use aes_gcm::aead::{Aead, NewAead};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::Rng;

mod gg20_sm_client;
use gg20_sm_client::join_computation;

#[derive(Debug, StructOpt)]
#[structopt(name = "gg20_keygen", about = "Multi-party ECDSA key generation tool")]
struct Cli {
    #[structopt(
        short,
        long,
        default_value = "http://localhost:8000/",
        help = "Address of the state machine manager"
    )]
    address: surf::Url,

    #[structopt(
        short,
        long,
        default_value = "default-keygen",
        help = "Room identifier for the computation"
    )]
    room: String,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "Path to save the encrypted output"
    )]
    output: PathBuf,

    #[structopt(short, long, help = "Index of this party")]
    index: u16,

    #[structopt(short, long, help = "Threshold for signature reconstruction")]
    threshold: u16,

    #[structopt(short, long, help = "Total number of parties")]
    number_of_parties: u16,
}

/// Execute the key generation protocol
async fn execute_keygen(
    address: surf::Url,
    room: &str,
    index: u16,
    threshold: u16,
    number_of_parties: u16,
) -> Result<LocalKey<Secp256k1>> {
    info!("Joining key generation computation room: {}", room);
    let (_i, incoming, outgoing) = join_computation(address, room)
        .await
        .context("Failed to join computation")?;

    let incoming = incoming.fuse();
    tokio::pin!(incoming);
    tokio::pin!(outgoing);

    info!("Initializing Keygen protocol");
    let keygen =
        Keygen::new(index, threshold, number_of_parties).context("Failed to initialize Keygen")?;

    info!("Running Keygen protocol");
    AsyncProtocol::new(keygen, incoming, outgoing)
        .run()
        .await
        .map_err(|e| anyhow!("Protocol execution failed: {}", e))
}

/// Encrypt the local key
fn encrypt_local_key(local_key: &LocalKey<Secp256k1>) -> Result<(Vec<u8>, [u8; 32], [u8; 12])> {
    let key = rand::thread_rng().gen::<[u8; 32]>();
    let nonce = rand::thread_rng().gen::<[u8; 12]>();

    let cipher = Aes256Gcm::new(Key::from_slice(&key));
    let serialized = serde_json::to_vec(local_key).context("Failed to serialize local key")?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), serialized.as_ref())
        .map_err(|e| anyhow!("Encryption failed: {:?}", e))?;

    Ok((encrypted, key, nonce))
}

/// Save the encrypted local key to a file
async fn save_encrypted_local_key(encrypted: &[u8], output_path: &PathBuf) -> Result<()> {
    debug!("Saving encrypted local key to file: {:?}", output_path);
    tokio::fs::write(output_path, encrypted)
        .await
        .context("Failed to save encrypted output to file")?;

    Ok(())
}

/// Display the public key in hexadecimal format
fn display_public_key(local_key: &LocalKey<Secp256k1>) {
    let public_key = local_key.public_key();
    let pk_bytes = public_key.to_bytes(false);
    info!("Generated Public Key (hex): {}", hex::encode(&*pk_bytes));
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    let args: Cli = Cli::from_args();
    info!("Starting key generation process");
    debug!("CLI arguments: {:?}", args);

    if args.threshold >= args.number_of_parties {
        warn!("Threshold must be less than the number of parties");
        return Err(anyhow!("Invalid threshold"));
    }

    // Execute key generation
    let local_key = execute_keygen(
        args.address,
        &args.room,
        args.index,
        args.threshold,
        args.number_of_parties,
    )
    .await?;

    info!("Key generation completed successfully");

    // Display the public key
    display_public_key(&local_key);

    // Encrypt the local key
    let (encrypted, key, nonce) = encrypt_local_key(&local_key)?;

    // Save the encrypted local key
    save_encrypted_local_key(&encrypted, &args.output).await?;

    info!("Encrypted local key saved to {:?}", args.output);
    println!("Encryption key (hex): {}", hex::encode(key));
    println!("Nonce (hex): {}", hex::encode(nonce));

    Ok(())
}
