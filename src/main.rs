use actix_web::{dev::ServiceRequest, web, App, Error, HttpResponse, HttpServer, Responder};
use actix_web_httpauth::{
    extractors::{bearer, AuthenticationError},
    middleware::HttpAuthentication,
};
use std::env;
use std::sync::Arc;

mod handlers;
mod secure_key;
mod utils;

// Health check endpoint to ensure the service is running.
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// Simple auth mechanics
async fn validator(
    req: ServiceRequest,
    credentials: bearer::BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    if credentials.token()
        == env::var("BearerToken")
            .unwrap_or_else(|_| "81ae70fd020f3e25938dde45acff2458".to_string())
    {
        Ok(req)
    } else {
        let config = req
            .app_data::<bearer::Config>()
            .cloned()
            .unwrap_or_default()
            .scope("urn:example:BearerAuth=Token");

        Err((AuthenticationError::from(config).into(), req))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init();

    // Load configuration from environment variables (with defaults)
    let host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("{}:{}", host, port);

    log::info!("Server starting at http://{}", address);

    // Initialize in-memory database (Sled)
    log::info!("Initializing in-memory database (Sled).");
    let db = sled::Config::new()
        .temporary(true)
        .open()
        .expect("Failed to open database");
    let db = Arc::new(db);

    // Start the Actix-Web server
    HttpServer::new(move || {
        let auth = HttpAuthentication::bearer(validator);
        App::new()
            .app_data(web::Data::new(db.clone()))
            .route("/health", web::get().to(health_check)) // Health check endpoint
            .wrap(auth) // Apply authentication middleware to the following routes
            .service(handlers::generate_key)
            .service(handlers::sign_message)
            .service(handlers::forget_key)
    })
    .bind(&address)?
    .run()
    .await
}
