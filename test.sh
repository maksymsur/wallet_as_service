#!/bin/bash

run_in_new_terminal() {
    gnome-terminal -- bash -c "$1"
}

# Create vault directories
mkdir -p vault/users vault/server

echo "Starting KMS trial key generation and signing process..."

# Start the state machine manager
run_in_new_terminal "
    echo 'Starting state machine manager...'
    RUST_LOG=info ./target/release/gg20_sm_manager
    read -p 'Press Enter to close this terminal...'
"

# Wait for the state machine manager to start
sleep 2

# Key Generation and Signing for all three parties
for i in {1..3}
do
    echo "Starting key generation and signing for party $i"
    
    # Determine the output directory based on party number
    if [ $i -le 2 ]; then
        OUTPUT_DIR="vault/users"
    else
        OUTPUT_DIR="vault/server"
    fi
    
    run_in_new_terminal "
        echo 'Running key generation for party $i...'
        OUTPUT=\$(RUST_LOG=info ./target/release/gg20_keygen -t 1 -n 3 -i $i --output $OUTPUT_DIR/local-share$i.json)
        echo \"\$OUTPUT\"
        
        # Extract key and nonce
        KEY=\$(echo \"\$OUTPUT\" | grep 'Encryption key (hex):' | cut -d ' ' -f 4)
        NONCE=\$(echo \"\$OUTPUT\" | grep 'Nonce (hex):' | cut -d ' ' -f 3)
        
        echo 'Key generation for party $i completed.'
        echo 'Key: '\$KEY
        echo 'Nonce: '\$NONCE
        echo 'Local share saved to: $OUTPUT_DIR/local-share$i.json'
        echo 'Press Enter to proceed with signing...'
        read
        
        echo 'Running signing process for party $i...'
        RUST_LOG=info RUST_BACKTRACE=1 ./target/release/gg20_signing -p 1,2 -d \"hello world\" -l $OUTPUT_DIR/local-share$i.json --key \$KEY --nonce \$NONCE
        
        echo 'Signing process for party $i completed. Press Enter to close this terminal...'
        read
    "
done

echo "All key generation and signing processes have been started."
echo "Please check the individual terminals for the progress and results of each party."
echo "The signing process will complete when all parties have finished their offline stage."

echo "KMS trial key generation and signing process initiated for all parties."
echo "Local shares for parties 1 and 2 are stored in vault/users/"
echo "Local share for party 3 (server) is stored in vault/server/"
