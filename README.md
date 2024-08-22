# Wallet as a service (WAAS) ad-hoc project to test some concepts for safe keys custom management

## Description

The WAAS project is a Blockchain backend wallet-as-a-service application built using Rust and the Actix-Web framework. The service includes functionalities for generating cryptographic keys, signing messages, and securely forgetting keys. 

**Architecture**: The implementation is modular, comprising multiple components such as handlers, secure key management, and utility functions. That is done intentionally as a good basis to foster further project development/testing by a team.

**Storage**: Security, code quality, and efficiency are emphasized, with an in-memory database (Sled) used to manage key storage. This is chosen as a quick in-RAM storage solution that shall be changed into more robust production-grade DB like PostgreSQL or smth like YugabyteDB (in case we need a multi-sharded horizontal scaling capabilities).

**Auth capabilities**: Bearer Token Authentication is implemented using `actix-web-httpauth`, this middleware ensures that only authorized users can access the core endpoints (`/generate-key`, `/sign-message`, `/forget-key`). The token is compared against an environment variable or a default value. This simple form of authentication is effective for a proof of concept but needs to be strengthened for production environments.

**Key Management**: SafeSecretKey abstraction wraps around the secp256k1::SecretKey, ensuring that sensitive cryptographic material is securely handled and zeroized upon drop. This approach demonstrates an awareness of the critical need to protect sensitive data from lingering in memory. That also lays some good ground for future expendability and key management depending on project development.

**Error Handling**: Custom Error Types like `AppError` and `SafeSecretKeyError` provide robust error categorization and response generation. Each error type is mapped to appropriate HTTP responses, enhancing the clarity and maintainability of error-handling logic.

## How to test

1. open 2 terminal windows
2. execute in the first window `RUST_LOG=info cargo run` and you'll see smth like this:
```
❯ RUST_LOG=info cargo run
   Compiling wallet_as_service v0.1.0 (/home/maksym/Desktop/wallet_as_service)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.79s                                                                            
     Running `target/debug/wallet_as_service`
[2024-08-21T16:11:47Z INFO  wallet_as_service] Server starting at http://127.0.0.1:8080
[2024-08-21T16:11:47Z INFO  wallet_as_service] Initializing in-memory database (Sled).
[2024-08-21T16:11:48Z INFO  actix_server::builder] starting 8 workers                                                                              
[2024-08-21T16:11:48Z INFO  actix_server::server] Actix runtime found; starting in Actix runtime
[2024-08-21T16:11:48Z INFO  actix_server::server] starting service: "actix-web-service-127.0.0.1:8080", workers: 8, listening on: 127.0.0.1:8080
[2024-08-21T16:11:51Z INFO  wallet_as_service::utils] Creating JSON response for key: key_id with value: 0a7dcc8d-52b8-4593-9332-c88fc528ac35
[2024-08-21T16:11:51Z INFO  wallet_as_service::utils] Creating JSON response for key: signature with value: 0525775f713ee04a24752eb67544339b75e0846deb8b6a7034ac6dc3cd98c4ad76c7507fdee45de107fe5ddeefef2bda74254291e62630de94a811414d34e3ee
``` 
3. when the server started to run - execute `bash test_script.sh` in the second window and observe the following:
```
❯ bash test_script.sh 
Generating a new key...
Generated key_id: 0a7dcc8d-52b8-4593-9332-c88fc528ac35
Signing the message...
Generated signature: 0525775f713ee04a24752eb67544339b75e0846deb8b6a7034ac6dc3cd98c4ad76c7507fdee45de107fe5ddeefef2bda74254291e62630de94a811414d34e3ee
Forgetting the key...
Key successfully forgotten.
Test completed successfully!
```

## Ideas for Improvement

**Enhanced Security:** Instead of a static Bearer Token, we might consider integrating OAuth2, JWT (JSON Web Tokens), or even mutual TLS for more secure authentication. While Sled is in-memory, we also might want to consider adding an encryption layer for stored data, especially if moving to persistent storage in production.

**Scalability & Persistence:** Transition from an in-memory database to a persistent store like PostgreSQL or a distributed key-value store (e.g., Redis) will allow for scaling out the service and persisting keys across service restarts. 

**Key Rotation & Expiry:** We also may implement key rotation policies and expiry mechanisms to ensure keys are not used indefinitely, enhancing security.

**Logging & Monitoring:** We need to enhance logging and implement tracing along with introduction of external monitoring tools like Prometheus, Jaeger, etc.

**Performance Optimization + Profiling & Flame-charts:** We need to systematically review all multi-threaded, async, I/O-bound operations (e.g., database access, cryptographic operations) and ensure they are fully asynchronous to prevent blocking the Actix-Web threads. Implement load testing to identify and address performance bottlenecks. Profiling and flame-charts allow better insight into the service's runtime performance and user behavior.

**Deployment Considerations:** We also need to containerize the application using Docker to simplify deployment and ensure consistency across environments. CI/CD Pipeline shall be set up allowing continuous integration and delivery + automate testing, building, and deployment.

**Improving the Secure Key Handling:** For high-security environments we might consider integrating with HSMs or using Rust libraries that interface with such modules for key generation and management. While zeroizing memory is critical, we also might consider additional safeguards like memory locking (using libraries like `mlock`) to prevent sensitive data from being swapped out to disk.

## Licenses & Vulnerabilities Checks

For better maintainability & safety the App dependencies are checked on every push:
- allowed Licenses via [cargo-deny](https://embarkstudios.github.io/cargo-deny/) by configuring `deny.toml`
- existing vulnerabilities vi [cargo-audit](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md)

Before using these dependencies install them: `cargo install cargo-audit && cargo install --locked cargo-deny`

The final audit result looks like this (warnings may be allowed in this respect):
```
wallet_as_service on  master [!+?] is  v0.1.0 via  v1.80.1 
❯ cargo fmt -- --check && cargo audit && cargo deny check && cargo test                                                                                   
    error[unlicensed]: ring = 0.16.20 is unlicensed
  ┌─ registry+https://github.com/rust-lang/crates.io-index#ring@0.16.20:2:9
  │                           
2 │ name = "ring"                   
  │         ━━━━ a valid license expression could not be retrieved for the crate
3 │ version = "0.16.20"
4 │ license = ""                                                                                        
  │            ─ license expression was not specified         
5 │ license-files = [
6 │     { path = "/home/maksym/.cargo/registry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/LICENSE", hash = 0xbd0eed23, score = 0.66, license = "OpenSSL" },                                                                                       
  │                                                                                                                                   ──── low confidence in the license text 
  │                                                             
  ├ ring v0.16.20                                                                                                                                  
    └── wallet_as_service v0.1.0                                             
      
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'                                                                                 
   ┌─ /home/maksym/Desktop/wallet_as_service/Cargo.lock:22:1                                                                                       
   │                                                                                                                                               
22 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index                                                                        
23 │ │ bitflags 2.6.0 registry+https://github.com/rust-lang/crates.io-index
   │ ╰────────────────────────────────────────────────────────────────────┘ lock entries
   │                                  
   ├ bitflags v1.3.2
     └── redox_syscall v0.2.16                      
         └── parking_lot_core v0.8.6                           
             └── parking_lot v0.11.2                                
                 └── sled v0.34.7
                     └── wallet_as_service v0.1.0   
```
and `carg-audit`

```
wallet_as_service on  master [!+?] is  v0.1.0 via  v1.80.1 
❯ cargo audit
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 648 security advisories (from /home/maksym/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (190 crate dependencies)
Crate:     secp256k1                
Version:   0.21.3                             
Warning:   unsound
Title:     Unsound API in `secp256k1` allows use-after-free and invalid deallocation from safe code     
Date:      2022-11-30                                         
ID:        RUSTSEC-2022-0070
URL:       https://rustsec.org/advisories/RUSTSEC-2022-0070                 
Dependency tree:                                                                                      
secp256k1 0.21.3
└── wallet_as_service 0.1.0

warning: 1 allowed warning found      
```
I left these vulnerabilities on purpose to better demonstrate the CI/CD process steps and indicate steps for improvement.

