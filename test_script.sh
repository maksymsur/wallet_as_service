#!/bin/bash

# Base URL of the API
BASE_URL="http://localhost:8080"

# Bearer token for authentication
BEARER_TOKEN="81ae70fd020f3e25938dde45acff2458"

# Generate a new key
echo "Generating a new key..."
generate_key_response=$(curl -s -X POST "${BASE_URL}/generate-key" \
  -H "Authorization: Bearer $BEARER_TOKEN")

key_id=$(echo $generate_key_response | jq -r '.key_id')

if [ "$key_id" == "null" ] || [ -z "$key_id" ]; then
  echo "Failed to generate key. Response: $generate_key_response"
  exit 1
fi

echo "Generated key_id: $key_id"

# Define a message to sign
MESSAGE="my test message"

# Sign the message using the generated key
echo "Signing the message..."
sign_message_response=$(curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $BEARER_TOKEN" \
  -d "{\"key_id\":\"$key_id\",\"message\":\"$MESSAGE\"}" \
  "${BASE_URL}/sign-message")

signature=$(echo $sign_message_response | jq -r '.signature')

if [ "$signature" == "null" ] || [ -z "$signature" ]; then
  echo "Failed to sign message. Response: $sign_message_response"
  exit 1
fi

echo "Generated signature: $signature"

# Forget the key
echo "Forgetting the key..."
forget_key_response=$(curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $BEARER_TOKEN" \
  -d "{\"key_id\":\"$key_id\"}" \
  "${BASE_URL}/forget-key")

if [ "$forget_key_response" == "Key forgotten" ]; then
  echo "Key successfully forgotten."
else
  echo "Failed to forget key. Response: $forget_key_response"
  exit 1
fi

echo "Test completed successfully!"
