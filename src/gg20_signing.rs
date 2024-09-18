use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::{debug, info, warn};
use structopt::StructOpt;

use curv::arithmetic::Converter;
use curv::elliptic::curves::secp256_k1::Secp256k1;
use curv::BigInt;

use multi_party_ecdsa::protocols::multi_party_ecdsa::gg_2020::state_machine::keygen::LocalKey;
use multi_party_ecdsa::protocols::multi_party_ecdsa::gg_2020::state_machine::sign::{
    CompletedOfflineStage, OfflineStage, SignManual,
};
use round_based::async_runtime::AsyncProtocol;
use round_based::Msg;

use aes_gcm::aead::{Aead, NewAead};
use aes_gcm::{Aes256Gcm, Key, Nonce};

mod gg20_sm_client;
use gg20_sm_client::join_computation;

mod paillier_validator;
use paillier_validator::{PaillierValidator, ValidationResult};

use surf;

/// Command-line interface structure for the GG20 signing tool
#[derive(Debug, StructOpt)]
#[structopt(name = "gg20_signing", about = "Multi-party ECDSA signing tool")]
struct Cli {
    /// Address of the state machine manager
    #[structopt(
        short,
        long,
        default_value = "http://localhost:8000/",
        help = "Address of the state machine manager"
    )]
    address: surf::Url,

    /// Room identifier for the computation
    #[structopt(
        short,
        long,
        default_value = "default-signing",
        help = "Room identifier for the computation"
    )]
    room: String,

    /// Path to the encrypted local share file
    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "Path to the encrypted local share file"
    )]
    local_share: PathBuf,

    /// Encryption key in hex format
    #[structopt(short, long, help = "Encryption key in hex format")]
    key: String,

    /// Nonce in hex format
    #[structopt(short, long, help = "Nonce in hex format")]
    nonce: String,

    /// Indices of participating parties
    #[structopt(
        short,
        long,
        use_delimiter(true),
        help = "Indices of participating parties"
    )]
    parties: Vec<u16>,

    /// Data to be signed
    #[structopt(short, long, help = "Data to be signed")]
    data_to_sign: String,
}

/// Read and decrypt the local share from a file
///
/// This function reads the encrypted local share from the specified file,
/// decrypts it using the provided key and nonce, and returns the decrypted
/// LocalKey<Secp256k1> object.
async fn read_local_share(path: &PathBuf, key: &[u8], nonce: &[u8]) -> Result<LocalKey<Secp256k1>> {
    info!("Reading encrypted local share from {:?}", path);
    // Read the encrypted data from the file
    let encrypted_data = tokio::fs::read(path)
        .await
        .context("Failed to read encrypted local share file")?;

    // Create a new AES-GCM cipher instance
    let cipher = Aes256Gcm::new(Key::from_slice(key));
    // Decrypt the data
    let decrypted = cipher
        .decrypt(Nonce::from_slice(nonce), encrypted_data.as_ref())
        .map_err(|e| anyhow!("Decryption failed: {:?}", e))?;

    // Parse the decrypted data into a LocalKey<Secp256k1> object
    serde_json::from_slice(&decrypted).context("Failed to parse decrypted local share")
}

/// Execute the offline stage of the signing protocol
///
/// This function performs the offline stage of the multi-party ECDSA signing protocol,
/// which includes joining the computation room, validating Paillier keys, and running
/// the OfflineStage protocol.
async fn execute_offline_stage(
    address: surf::Url,
    room: &str,
    parties: Vec<u16>,
    local_share: LocalKey<Secp256k1>,
) -> Result<CompletedOfflineStage> {
    info!("Joining offline computation room: {}-offline", room);
    // Join the computation room for the offline stage
    let (i, incoming, outgoing) = join_computation(address, &format!("{}-offline", room))
        .await
        .context("Failed to join offline computation")?;

    let incoming = incoming.fuse();
    tokio::pin!(incoming);
    tokio::pin!(outgoing);

    // Create a PaillierValidator instance for validating Paillier keys
    let validator = PaillierValidator::new(1 << 20);
    info!("PaillierValidator is initiated");

    // Validate all Paillier modulus NNs before proceeding
    for (idx, key) in local_share.paillier_key_vec.iter().enumerate() {
        match validator.validate_modulus(&key.nn) {
            Ok(ValidationResult::Valid) => {
                info!("Paillier modulus for party {} validation passed", idx);
            }
            Err(e) => {
                return Err(anyhow!(
                    "Invalid Paillier modulus for party {} with error {}",
                    idx,
                    e
                ));
            }
        }
    }

    info!("Initializing OfflineStage");
    // Create a new OfflineStage instance
    let signing = OfflineStage::new(i, parties, local_share)?;

    info!("Running OfflineStage protocol");
    // Execute the OfflineStage protocol
    AsyncProtocol::new(signing, incoming, outgoing)
        .run()
        .await
        .map_err(|e| anyhow!("Offline protocol execution failed: {}", e))
}

/// Execute the online stage of the signing protocol
///
/// This function performs the online stage of the multi-party ECDSA signing protocol,
/// which includes joining the computation room, initializing the SignManual object,
/// exchanging partial signatures, and completing the signing process.
async fn execute_online_stage(
    address: surf::Url,
    room: &str,
    data_to_sign: &str,
    completed_offline_stage: CompletedOfflineStage,
    number_of_parties: usize,
) -> Result<String> {
    info!("Joining online computation room: {}-online", room);
    // Join the computation room for the online stage
    let (i, incoming, outgoing) = join_computation(address, &format!("{}-online", room))
        .await
        .context("Failed to join online computation")?;

    tokio::pin!(incoming);
    tokio::pin!(outgoing);

    info!("Initializing SignManual");
    // Create a new SignManual instance
    let (signing, partial_signature) = SignManual::new(
        BigInt::from_bytes(data_to_sign.as_bytes()),
        completed_offline_stage,
    )?;

    debug!("Sending partial signature");
    // Send the partial signature to other parties
    outgoing
        .send(Msg {
            sender: i,
            receiver: None,
            body: partial_signature,
        })
        .await
        .context("Failed to send partial signature")?;

    info!("Collecting partial signatures from other parties");
    // Collect partial signatures from other parties
    let partial_signatures: Vec<_> = incoming
        .take(number_of_parties - 1)
        .map_ok(|msg| msg.body)
        .try_collect()
        .await
        .context("Failed to collect partial signatures")?;

    info!("Completing the signing process");
    // Complete the signing process by combining all partial signatures
    let signature = signing
        .complete(&partial_signatures)
        .context("Failed to complete online stage")?;

    // Serialize the final signature
    serde_json::to_string(&signature).context("Failed to serialize signature")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Parse command-line arguments
    let args: Cli = Cli::from_args();
    info!("Starting signing process");
    debug!("CLI arguments: {:?}", args);

    // Decode the encryption key and nonce from hex
    let key = hex::decode(&args.key).context("Failed to decode encryption key")?;
    let nonce = hex::decode(&args.nonce).context("Failed to decode nonce")?;

    // Read and decrypt the local share
    let local_share = read_local_share(&args.local_share, &key, &nonce).await?;

    let number_of_parties = args.parties.len();
    debug!("Number of participating parties: {}", number_of_parties);

    // Ensure there are at least 2 parties for multi-party signing
    if number_of_parties < 2 {
        warn!("At least 2 parties are required for multi-party signing");
        return Err(anyhow!("Insufficient number of parties"));
    }

    // Execute the offline stage of the signing protocol
    let completed_offline_stage =
        execute_offline_stage(args.address.clone(), &args.room, args.parties, local_share).await?;

    info!("Offline stage completed successfully");

    // Execute the online stage of the signing protocol
    let signature = execute_online_stage(
        args.address,
        &args.room,
        &args.data_to_sign,
        completed_offline_stage,
        number_of_parties,
    )
    .await?;

    // Print the final signature
    println!("Signature: {}", signature);

    info!("Signing process completed successfully");
    Ok(())
}
