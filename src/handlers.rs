use actix_web::{post, web, HttpResponse, ResponseError};
use ring::digest::{Context, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use secp256k1::{Message, Secp256k1, SecretKey};
use serde::Deserialize;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::secure_key::{SafeSecretKey, SafeSecretKeyError};
use crate::utils::hex_response;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Internal Server Error: {0}")]
    InternalServerError(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Key Handling Error: {0}")]
    KeyHandlingError(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::InternalServerError(ref message) => {
                HttpResponse::InternalServerError().body(message.clone())
            }
            AppError::BadRequest(ref message) => HttpResponse::BadRequest().body(message.clone()),
            AppError::NotFound(ref message) => HttpResponse::NotFound().body(message.clone()),
            AppError::KeyHandlingError(ref message) => {
                HttpResponse::InternalServerError().body(message.clone())
            }
        }
    }
}

// Implement conversion from SafeSecretKeyError to AppError
impl From<SafeSecretKeyError> for AppError {
    fn from(e: SafeSecretKeyError) -> Self {
        AppError::KeyHandlingError(format!("{:?}", e))
    }
}

#[post("/generate-key")]
async fn generate_key(db: web::Data<Arc<sled::Db>>) -> Result<HttpResponse, AppError> {
    let key_id = Uuid::new_v4().to_string();

    // Generate a new random secret key using ring for secure randomness
    let rng = SystemRandom::new();
    let mut secret_key_bytes = [0u8; 32];
    rng.fill(&mut secret_key_bytes).map_err(|_| {
        AppError::KeyHandlingError("Failed to generate random bytes for secret key".into())
    })?;

    // Create a SecretKey and then SafeSecretKey
    let secret_key = SecretKey::from_slice(&secret_key_bytes)
        .map_err(|_| AppError::KeyHandlingError("Failed to create secret key".into()))?;

    // We check that the conversion is successfully performed and drop the key afterwards
    // The concept of SafeSecretKey is brought as a ground for extending the Service further
    // and making different safe manipulations with that struct
    let safe_key = SafeSecretKey::try_from(&secret_key)?;
    drop(safe_key);

    // Store the raw key bytes directly in the database
    db.insert(key_id.as_bytes(), &secret_key_bytes)
        .map_err(|e| {
            log::error!("Failed to store key: {:?}", e);
            AppError::InternalServerError("Failed to store key".into())
        })?;

    // Zeroize the key bytes after use
    secret_key_bytes.zeroize();

    Ok(HttpResponse::Ok().json(hex_response("key_id", &key_id)))
}

#[derive(Deserialize)]
struct SignMessageRequest {
    key_id: String,
    message: String,
}

#[post("/sign-message")]
async fn sign_message(
    db: web::Data<Arc<sled::Db>>,
    req: web::Json<SignMessageRequest>,
) -> Result<HttpResponse, AppError> {
    // Validate the key_id format (UUID in this case)
    let _ = Uuid::parse_str(&req.key_id)
        .map_err(|_| AppError::BadRequest("Invalid key_id format".into()))?;

    // Retrieve the key bytes from the database
    let key_data = db
        .get(req.key_id.as_bytes())
        .map_err(|e| {
            log::error!("Failed to read key: {:?}", e);
            AppError::InternalServerError("Failed to read key".into())
        })?
        .ok_or_else(|| AppError::NotFound("Key not found".into()))?
        .to_vec();

    // Convert the key bytes to SafeSecretKey
    let secret_key = SecretKey::from_slice(&key_data)
        .map_err(|_| AppError::KeyHandlingError("Failed to recreate SecretKey".into()))?;

    let key = SafeSecretKey::try_from(&secret_key)?;

    // Hash the message using SHA-256
    let mut context = Context::new(&SHA256);
    context.update(req.message.as_bytes());
    let message_hash = context.finish();

    // Create a Secp256k1 message from the hash
    let message = Message::from_slice(message_hash.as_ref())
        .map_err(|_| AppError::BadRequest("Invalid message format after hashing".into()))?;

    // Sign the message
    let secp = Secp256k1::new();
    let signature = secp.sign_ecdsa(&message, &key);

    Ok(HttpResponse::Ok().json(hex_response(
        "signature",
        &hex::encode(signature.serialize_compact()),
    )))
}

#[derive(Deserialize)]
struct ForgetKeyRequest {
    key_id: String,
}

#[post("/forget-key")]
async fn forget_key(
    db: web::Data<Arc<sled::Db>>,
    req: web::Json<ForgetKeyRequest>,
) -> Result<HttpResponse, AppError> {
    // Validate the key_id format (UUID in this case)
    let _ = Uuid::parse_str(&req.key_id)
        .map_err(|_| AppError::BadRequest("Invalid key_id format".into()))?;

    // Attempt to remove the key from the database
    let removed_key = db.remove(req.key_id.as_bytes()).map_err(|e| {
        log::error!("Failed to remove key: {:?}", e);
        AppError::InternalServerError("Failed to remove key".into())
    })?;

    // Check if a key was actually removed (indicating it existed)
    if removed_key.is_none() {
        return Err(AppError::NotFound("Key not found".into()));
    }

    Ok(HttpResponse::Ok().body("Key forgotten"))
}
