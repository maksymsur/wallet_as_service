# Multi-Party ECDSA Key Generation and Signing

This project implements a multi-party ECDSA (Elliptic Curve Digital Signature Algorithm) key generation and signing protocol based on the GG20 scheme. It provides a distributed way to generate and manage ECDSA keys, as well as create signatures without any single party having access to the full private key.

## Overview

This implementation allows multiple parties to collaboratively generate an ECDSA key pair and create signatures without revealing their individual secret shares. It uses a threshold scheme, where a specified number of parties must participate to complete the signing process.

## Features

- Distributed ECDSA key generation
- Threshold-based multi-party signing
- Secure communication between parties
- State machine management for coordinating the protocol
- Paillier key validation for enhanced security

## Components

The project consists of several main components:

1. `gg20_keygen`: Handles the distributed key generation process
2. `gg20_signing`: Implements the multi-party signing protocol
3. `gg20_sm_manager`: Manages the state machine and communication between parties
4. `gg20_sm_client`: Provides client-side functionality for connecting to the state machine manager
5. `paillier_validator`: Validates Paillier keys to prevent potential vulnerabilities

## Installation

To install and set up the project, follow these steps:

1. Ensure you have Rust and Cargo installed on your system.
2. Clone the repository:

```bash
git clone https://github.com/your-repo/multi-party-ecdsa.git
cd multi-party-ecdsa
```
3. Build the project:

```bash
cargo build --release --all
```

## Usage

### State Machine Manager

To start the state machine manager, which coordinates communication between parties:

```bash
./target/release/gg20_sm_manager
```
By default, this will start the manager on `http://localhost:8000`.

### Key Generation

To generate keys for a party, use the `gg20_keygen` binary:

```bash
./target/release/gg20_keygen -t <threshold> -n <total_parties> -i <party_index> --output <output_path>
```

This will generate a key share for party 1 in a 2-of-3 threshold scheme.

### Signing

To participate in a signing process, use the `gg20_signing` binary:

```bash
./target/release/gg20_signing -p <participating_parties> -d <data_to_sign> -l <local_share_path> --key <encryption_key> --nonce <encryption_nonce>
```
This will initiate or participate in a signing process for the message "Hello, World!" using parties 1 and 2.

### Automation

All the steps for starting SM, key generation and signing may be executed by the script:

```bash
bash ./test.sh

```

## Security Considerations

- This implementation includes a Paillier key validator to prevent potential vulnerabilities related to maliciously crafted moduli.
- Ensure that you use secure channels for communication between parties and the state machine manager in production environments.
- Safeguard the encrypted local shares and their corresponding encryption keys and nonces.
- Regularly update dependencies to incorporate security patches.

## Licenses & Vulnerabilities Checks

For better maintainability & safety the App dependencies are checked on every push:
- allowed Licenses via [cargo-deny](https://embarkstudios.github.io/cargo-deny/) by configuring `deny.toml`
- existing vulnerabilities vi [cargo-audit](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md)

Before using these dependencies install them: `cargo install cargo-audit && cargo install --locked cargo-deny`

The final audit result looks like this (warnings may be allowed in this respect):

```bash
❯ cargo cargo audit
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 659 security advisories (from /home/maksym/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (481 crate dependencies)
Crate:     curve25519-dalek
Version:   3.2.0
Title:     Timing variability in `curve25519-dalek`'s `Scalar29::sub`/`Scalar52::sub`
Date:      2024-06-18
ID:        RUSTSEC-2024-0344
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0344
Solution:  Upgrade to >=4.1.3
Dependency tree:
curve25519-dalek 3.2.0
└── curv-kzen 0.9.0
    ├── zk-paillier 0.4.3
    │   ├── multi-party-ecdsa 0.8.1
    │   │   └── kms_trial 0.1.0
    │   └── kms_trial 0.1.0
    ├── multi-party-ecdsa 0.8.1
    ├── kzen-paillier 0.4.2
    │   ├── zk-paillier 0.4.3
    │   ├── multi-party-ecdsa 0.8.1
    │   └── kms_trial 0.1.0
```
and 

```bash
❯ cargo deny check           
warning[duplicate]: found 2 duplicate entries for crate 'aead'
  ┌─ /home/maksym/Desktop/trial/kms_trial/Cargo.lock:3:1                                    
  │                                                         
3 │ ╭ aead 0.3.2 registry+https://github.com/rust-lang/crates.io-index
4 │ │ aead 0.4.3 registry+https://github.com/rust-lang/crates.io-index  
  │ ╰────────────────────────────────────────────────────────────────┘ lock entries                     
  │  
  ├ aead v0.3.2            
    └── aes-gcm v0.8.0                                          
        └── cookie v0.14.4                                  
            └── http-types v2.12.0                                      
                ├── async-sse v5.1.0                                  
                │   └── kms_trial v0.1.0                     
                ├── http-client v6.5.3
                │   └── surf v2.3.2                                                 
                │       └── kms_trial v0.1.0 (*)                                     
                └── surf v2.3.2 (*)                                              
  ├ aead v0.4.3
    └── aes-gcm v0.9.4                                                           
        └── kms_trial v0.1.0        
```

## Ideas for Improvement

1. Enhanced Paillier modulus validation
2. Integration with hardware security modules (HSMs)
3. Support for additional cryptographic schemes
4. User-friendly interfaces for key management

## Contributing

Contributions to this project are welcome. Please follow these steps:

1. Fork the repository
2. Create a new branch for your feature or bug fix
3. Make your changes and write tests if applicable
4. Submit a pull request with a clear description of your changes

## License

MIT

---

For more detailed information on each component, please refer to the individual source files and comments within the code.
