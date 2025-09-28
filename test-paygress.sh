#!/bin/bash

# =============================================================================
# Paygress NIP-04 Testing Script
# =============================================================================
# Usage: ./test-paygress.sh [SERVICE_NPUB]
# If SERVICE_NPUB is not provided, the script will prompt for it
# This script tests NIP-04 (Encrypted Direct Messages) functionality

SERVICE_NPUB_INPUT="$1"

# =============================================================================
# SCRIPT CONFIGURATION AND VALIDATION
# =============================================================================
validate_npub() {
    local npub="$1"
    # Check if npub starts with npub1 and has appropriate length
    if [[ ! "$npub" =~ ^npub1[a-zA-Z0-9]{50,100}$ ]]; then
        return 1
    fi
    return 0
}

# Get service public key from command line argument or prompt user
if [ -z "$SERVICE_NPUB_INPUT" ]; then
    echo "📡 Paygress Service Public Key Required"
    echo "====================================="
    echo "Please provide the service public key (npub1... format)"
    echo "You can get this by running: kubectl logs -n ingress-system -l app=paygress-sidecar"
    echo ""
    read -p "Enter service public key (npub1...): " SERVICE_NPUB_INPUT
    
    if [ -z "$SERVICE_NPUB_INPUT" ]; then
        echo "❌ Error: Service public key is required"
        exit 1
    fi
fi

# Validate service public key format
if ! validate_npub "$SERVICE_NPUB_INPUT"; then
    echo "❌ Error: Invalid service public key format"
    echo "Key should start with 'npub1' followed by base32 characters"
    exit 1
fi

SERVICE_NPUB="$SERVICE_NPUB_INPUT"
# This script demonstrates the complete NIP-04 workflow for testing the Paygress system:
# 1. Generate user keypair for Nostr communication
# 2. Configure service public key
# 3. Generate Cashu payment tokens using CDK CLI
# 4. Send NIP-04 encrypted direct message to provision a pod
# 5. Listen for NIP-04 encrypted response with access credentials
# 6. Parse response to get SSH access details (nak auto-decrypts)
# 7. Connect to the provisioned pod via SSH

# Requirements:
# - nak (Nostr CLI tool)
# - jq (JSON processor)
# - cdk-cli (Cashu CDK CLI)
# - ssh (SSH client)

echo "🚀 Paygress Testing Script"
echo "=========================="
echo ""

# =============================================================================
# 1. USER KEYPAIR GENERATION
# =============================================================================
# Generate a new Nostr keypair for this test session
echo "🔐 Step 1: Generating User Keypair"
echo "----------------------------------"

# Generate user keys using nak (safer - no manual key handling)
USER_PRIVATE_HEX=$(nak key generate)
echo "Generated private key (hex): $USER_PRIVATE_HEX"

# Convert hex to bech32 format (nsec1... for private key)
USER_NSEC=$(echo "$USER_PRIVATE_HEX" | nak encode nsec)
echo "User private key (bech32/nsec): $USER_NSEC"

# Get public key from private key (use hex, not bech32)
USER_NPUB_HEX=$(nak key public "$USER_PRIVATE_HEX")
echo "User public key (hex): $USER_NPUB_HEX"

# Convert public key to bech32 format (npub1... for public key)
USER_NPUB_BECH32=$(echo "$USER_NPUB_HEX" | nak encode npub)
echo "User public key (bech32/npub): $USER_NPUB_BECH32"

# Export keys for use in subsequent commands
export NSEC="$USER_NSEC"
export NPUB="$USER_NPUB_BECH32"

echo "✅ User keypair generated and exported"
echo ""

# =============================================================================
# 2. SERVICE PUBLIC KEY CONFIGURATION
# =============================================================================
# Configure the service public key
echo "📡 Step 2: Configuring Service Public Key"
echo "-----------------------------------------"

echo "Service public key (bech32): $SERVICE_NPUB"

# Convert npub to hex format for the p tag
echo "Converting npub to hex format..."
SERVICE_PUBKEY_HEX=$(nak decode --pubkey "$SERVICE_NPUB" 2>/dev/null)
if [ -z "$SERVICE_PUBKEY_HEX" ]; then
    echo "❌ Error: Failed to convert npub to hex format"
    echo "Make sure nak is installed and the npub format is correct"
    exit 1
fi

echo "DEBUG: Service public key (bech32): $SERVICE_NPUB"
echo "DEBUG: Service public key (hex): $SERVICE_PUBKEY_HEX"
echo "✅ Successfully converted npub to hex format"

echo "✅ Service public key configured"
echo ""

# =============================================================================
# 3. CASHU TOKEN GENERATION USING CDK CLI
# =============================================================================
# Generate Cashu payment tokens for pod provisioning
echo "💰 Step 3: Generating Cashu Payment Tokens"

# Get tokens from test mint (1000 sats = 1000 minutes = ~16 hours)
echo "Getting Cashu tokens from test mint..."

# Check wallet balance and mint tokens if needed
echo "Checking wallet balance..."
BALANCE_OUTPUT=$(cdk-cli balance 2>&1)
BALANCE_EXIT_CODE=$?

# Extract the total balance (last number from the "Total balance" line)
WALLET_BALANCE=$(echo "$BALANCE_OUTPUT" | grep "Total balance" | awk '{print $(NF-1)}' | sed 's/,//g')

# If we couldn't get the balance from the output, try a different approach
if [ -z "$WALLET_BALANCE" ]; then
    # Try to get balance directly without grep/awk
    WALLET_BALANCE=$(echo "$BALANCE_OUTPUT" | tail -1 | sed 's/[^0-9]*\([0-9]*\).*/\1/')
fi

# If we still don't have a balance or balance is 0, mint some tokens
if [ -z "$WALLET_BALANCE" ] || [ "$WALLET_BALANCE" -eq 0 ] 2>/dev/null; then
    echo "⚠️  Warning: Wallet balance is 0 or could not be determined"
    echo "DEBUG: Balance command exit code: $BALANCE_EXIT_CODE"
    echo "DEBUG: Balance output: $BALANCE_OUTPUT"
    echo "Minting 1000 sat tokens from test mint..."
    
    # Mint 1000 sat tokens
    MINT_OUTPUT=$(cdk-cli mint https://nofees.testnut.cashu.space 1000 2>&1)
    MINT_EXIT_CODE=$?
    
    if [ $MINT_EXIT_CODE -ne 0 ]; then
        echo "❌ Error: Failed to mint tokens"
        echo "DEBUG: Mint command exit code: $MINT_EXIT_CODE"
        echo "DEBUG: Mint output: $MINT_OUTPUT"
        exit 1
    fi
    
    echo "✅ Successfully minted 1000 sat tokens"
    
    # Check balance again after minting
    BALANCE_OUTPUT=$(cdk-cli balance 2>&1)
    WALLET_BALANCE=$(echo "$BALANCE_OUTPUT" | grep "Total balance" | awk '{print $(NF-1)}' | sed 's/,//g')
    
    # If we still can't get the balance, try the alternative approach
    if [ -z "$WALLET_BALANCE" ]; then
        WALLET_BALANCE=$(echo "$BALANCE_OUTPUT" | tail -1 | sed 's/[^0-9]*\([0-9]*\).*/\1/')
    fi
    
    echo "Current wallet balance after minting: $WALLET_BALANCE sat"
else
    echo "Current wallet balance: $WALLET_BALANCE sat"
fi

# Use 500 sat or the available balance if less than 500
if [ "$WALLET_BALANCE" -lt 500 ]; then
    TOKEN_AMOUNT=$WALLET_BALANCE
    echo "Using available balance: $TOKEN_AMOUNT sat"
else
    TOKEN_AMOUNT=500
fi

echo "Creating spendable Cashu token for $TOKEN_AMOUNT sat..."
# Create a temporary file to capture the output
TEMP_FILE=$(mktemp)
echo "$TOKEN_AMOUNT" | cdk-cli send --memo "Paygress test token" > "$TEMP_FILE" 2>&1
# Extract just the token line (should start with cashuB)
CASHU_TOKEN=$(grep "^cashuB" "$TEMP_FILE" | head -1)
# Clean up the temporary file
rm "$TEMP_FILE"

if [ -z "$CASHU_TOKEN" ]; then
    echo "❌ Failed to generate Cashu token"
    exit 1
fi

if [ -z "$CASHU_TOKEN" ]; then
    echo "❌ Failed to generate Cashu token"
    exit 1
fi

echo "Generated Cashu token: [REDACTED - token is too long to display]"
echo "✅ Cashu token generated successfully"
echo ""

# =============================================================================
# 4. CREATE ENCRYPTED NOSTR REQUEST (NIP-04)
# =============================================================================
# Create and send NIP-04 encrypted Nostr request to provision a pod
echo "📨 Step 4: Creating and Sending NIP-04 Encrypted Nostr Request"
echo "--------------------------------------------------------------"

# Create your request JSON with payment token and pod configuration
REQUEST_JSON="{\"cashu_token\":\"$CASHU_TOKEN\",\"pod_spec_id\":\"standard\",\"pod_image\":\"linuxserver/openssh-server:latest\",\"ssh_username\":\"alice\",\"ssh_password\":\"my_secure_password\"}"

echo "Request JSON: $REQUEST_JSON"

# Encrypt the request using nak with NIP-44 encryption
echo "Encrypting request..."
# Debug: Show the inputs to the encryption command
echo "DEBUG: Request JSON length: ${#REQUEST_JSON}"
echo "DEBUG: User private key: $NSEC"
echo "DEBUG: Service public key (hex): $SERVICE_PUBKEY_HEX"

# Send NIP-04 encrypted direct message using nak
echo "Sending NIP-04 encrypted direct message..."
echo "DEBUG: Request JSON: $REQUEST_JSON"
echo "DEBUG: User private key: $NSEC"
echo "DEBUG: Service public key (npub): $SERVICE_NPUB"
echo "DEBUG: Service public key (hex): $SERVICE_PUBKEY_HEX"

# Encrypt the content manually using NIP-04 to ensure compatibility
echo "Encrypting content with NIP-04..."
ENCRYPTED_CONTENT=$(nak encrypt --nip04 --sec "$NSEC" --recipient-pubkey "$SERVICE_PUBKEY_HEX" "$REQUEST_JSON" 2>/dev/null)
if [ -z "$ENCRYPTED_CONTENT" ]; then
    echo "❌ Error: Failed to encrypt content with NIP-04"
    exit 1
fi

echo "✅ Content encrypted successfully"
echo "DEBUG: Encrypted content length: ${#ENCRYPTED_CONTENT}"

# Send NIP-04 encrypted direct message with manually encrypted content
EVENT_ID=$(nak event \
  --sec "$NSEC" \
  --kind 4 \
  -p "$SERVICE_PUBKEY_HEX" \
  --content "$ENCRYPTED_CONTENT" \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band 2>&1)
SEND_EXIT_CODE=$?

# Debug: Show the send output and exit code
echo "DEBUG: Send exit code: $SEND_EXIT_CODE"
echo "DEBUG: Event ID: $EVENT_ID"

# Extract the actual event ID from the JSON output
ACTUAL_EVENT_ID=$(echo "$EVENT_ID" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

# Check if send was successful
if [ $SEND_EXIT_CODE -ne 0 ]; then
    echo "❌ Error: Failed to send NIP-04 encrypted message"
    echo "Send command failed with exit code: $SEND_EXIT_CODE"
    echo "Send output: $EVENT_ID"
    exit 1
fi

# Check for error patterns in output (POSIX compatible)
case "$EVENT_ID" in
    *"error"*|*"Error"*|*"failed"*|*"invalid"*)
        echo "❌ Error: Failed to send NIP-04 encrypted message"
        echo "Send output contains error: $EVENT_ID"
        exit 1
        ;;
esac

# Check if we got a valid event ID
if [ -z "$ACTUAL_EVENT_ID" ]; then
    echo "❌ Error: Could not extract event ID from output"
    echo "Send output: $EVENT_ID"
    exit 1
fi

echo "✅ NIP-04 encrypted message sent successfully"
echo "Event ID: $ACTUAL_EVENT_ID"
echo ""

# =============================================================================
# 5. LISTEN FOR NIP-04 ENCRYPTED RESPONSE
# =============================================================================
# Listen for NIP-04 encrypted response from the service with access credentials
echo "👂 Step 5: Listening for NIP-04 Encrypted Response"
echo "--------------------------------------------------"

# Create a temporary file to store responses
RESPONSE_FILE=$(mktemp)
echo "Storing responses in: $RESPONSE_FILE"

echo "Listening for NIP-04 encrypted direct message (kind 4) from service..."
echo "This will timeout after 120 seconds if no response is received"
echo "Looking for responses with SSH access details from the container..."

# Listen for NIP-04 encrypted direct messages (kind 4) with a timeout
# nak will automatically decrypt messages for us
timeout 120 nak req --sec "$NSEC" --kind 4 --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band > "$RESPONSE_FILE" 2>&1 &
LISTEN_PID=$!

# Wait for the background process to complete or timeout
wait $LISTEN_PID

echo ""
echo "Response file content:"
cat "$RESPONSE_FILE"
echo ""

# Check if we received any responses
if [ ! -s "$RESPONSE_FILE" ]; then
    echo "⚠️  No responses received within timeout period"
    rm "$RESPONSE_FILE"
    exit 1
fi

# Filter responses to find the one with SSH access details
# We're looking for events with content that contains "access_details" and SSH connection info
# The response will come from a new npub (the container we just bought)
PROCESSED_RESPONSE_FILE=$(mktemp)
# Extract only valid JSON lines (starting with {) and filter for paygress responses
grep "^{" "$RESPONSE_FILE" | while read -r line; do
    # Check if the event has the right tags and content structure
    if echo "$line" | jq -e '.tags[] | select(.[0] == "t" and (.[1] == "paygress" or .[1] == "response"))' >/dev/null 2>&1; then
        # Check if content contains access_details
        CONTENT=$(echo "$line" | jq -r '.content' 2>/dev/null)
        if echo "$CONTENT" | jq -e '.kind == "access_details"' >/dev/null 2>&1; then
            echo "$line"
        fi
    fi
done > "$PROCESSED_RESPONSE_FILE"

# Check if we have any valid paygress response events
if [ ! -s "$PROCESSED_RESPONSE_FILE" ]; then
    echo "⚠️  No valid paygress response events found in response file"
    echo "Showing all JSON events for debugging:"
    grep "^{" "$RESPONSE_FILE" | head -5
    rm "$RESPONSE_FILE" "$PROCESSED_RESPONSE_FILE"
    exit 1
fi

# Use the first valid paygress response
FIRST_RESPONSE=$(head -1 "$PROCESSED_RESPONSE_FILE")

if [ -z "$FIRST_RESPONSE" ]; then
    echo "❌ No valid paygress response found in response file"
    rm "$RESPONSE_FILE" "$PROCESSED_RESPONSE_FILE"
    exit 1
fi

# Get the sender's pubkey from the response
SENDER_PUBKEY=$(echo "$FIRST_RESPONSE" | jq -r '.pubkey')
echo "Response received from pubkey: $SENDER_PUBKEY"
echo "Note: This should be different from the service pubkey, as it comes from the container"

# Save the first response to a dedicated file for debugging
echo "$FIRST_RESPONSE" > "${RESPONSE_FILE}.first"
echo "First paygress response saved to: ${RESPONSE_FILE}.first"

# Clean up temporary files
rm "$RESPONSE_FILE" "$PROCESSED_RESPONSE_FILE"

echo "✅ Paygress response received and filtered"
echo ""

# =============================================================================
# 6. PARSE NIP-04 RESPONSE
# =============================================================================
# Parse the NIP-04 response (nak automatically decrypts for us)
echo "🔓 Step 6: Parsing NIP-04 Response"
echo "----------------------------------"

# Use the first response from the response file
RESPONSE_JSON="$FIRST_RESPONSE"

echo "Parsing response from service..."
# Extract the content field from the JSON response (already decrypted by nak)
DECRYPTED_RESPONSE=$(echo "$RESPONSE_JSON" | jq -r '.content')

if [ -z "$DECRYPTED_RESPONSE" ] || [ "$DECRYPTED_RESPONSE" = "null" ]; then
    echo "❌ Error: Failed to parse response content"
    echo "Make sure you have the correct response from the service"
    echo "The response should come from the Paygress service"
    exit 1
fi

echo "Decrypted response:"
echo "$DECRYPTED_RESPONSE" | jq '.'

# Extract SSH connection details from the decrypted response
if echo "$DECRYPTED_RESPONSE" | jq -e .pod_name >/dev/null 2>&1; then
    POD_NAME=$(echo "$DECRYPTED_RESPONSE" | jq -r '.pod_name')
    SSH_USERNAME=$(echo "$DECRYPTED_RESPONSE" | jq -r '.ssh_username')
    SSH_PASSWORD=$(echo "$DECRYPTED_RESPONSE" | jq -r '.ssh_password')
    NODE_PORT=$(echo "$DECRYPTED_RESPONSE" | jq -r '.node_port')
    
    echo ""
    echo "📋 SSH Access Details:"
    echo "   Pod Name: $POD_NAME"
    echo "   SSH Username: $SSH_USERNAME"
    echo "   SSH Password: $SSH_PASSWORD"
    echo "   Node Port: $NODE_PORT"
    echo ""
else
    echo "⚠️  SSH connection details not found in response"
    echo "   The response format might be different than expected"
fi

echo "✅ Response decrypted successfully"
echo ""

# =============================================================================
# 7. SSH ACCESS TO POD
# =============================================================================
# Connect to the provisioned pod via SSH
echo "🔗 Step 7: SSH Access to Pod"
echo "-----------------------------"

# Check if we have the necessary SSH connection details
if [ -n "$POD_NAME" ] && [ -n "$SSH_USERNAME" ] && [ -n "$SSH_PASSWORD" ] && [ -n "$NODE_PORT" ]; then
    echo "Connecting to pod via SSH..."
    echo ""
    echo "SSH Connection Command:"
    echo "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no $SSH_USERNAME@\$(minikube ip) -p $NODE_PORT"
    echo ""
    echo "Password: $SSH_PASSWORD"
    echo ""
    echo "To connect, run the above command in a new terminal."
else
    echo "⚠️  SSH connection details not available in response."
    echo "   You may need to extract them manually from the decrypted response above."
fi

echo "✅ Testing script completed"
echo ""

# =============================================================================
# USAGE INSTRUCTIONS
# =============================================================================
echo "📋 Usage Instructions:"
echo "---------------------"
echo "1. Make the script executable: chmod +x test-paygress.sh"
echo "2. Run the script with service key: ./test-paygress.sh npub1your_service_key_here"
echo "3. Or run without arguments and enter key when prompted: ./test-paygress.sh"
echo "4. Follow the prompts and replace placeholder values with actual ones"
echo "5. Monitor the Nostr relays for responses"
echo ""
echo "🔧 Important Notes:"
echo "------------------"
echo "- Replace ENCRYPTED_RESPONSE with the actual encrypted response content"
echo "- Ensure CDK CLI is built before running: cd ../cdk && cargo build --release"
echo "- Make sure you have sufficient Cashu tokens for the requested duration"
echo "- Pod duration is based on payment: 1 sat = 1 minute"

echo ""
echo "🔧 Advanced Response Filtering:"
echo "-----------------------------"
echo "- In a production implementation, you would filter responses by:"
echo "  1. Including a unique request ID in your request"
echo "  2. Looking for responses that reference that request ID"
echo "  3. Verifying the sender is the container you provisioned"
echo ""
echo "🎉 Paygress NIP-04 testing workflow completed!"
echo ""
echo "📋 NIP-04 Testing Commands Summary:"
echo "=================================="
echo ""
echo "1. Generate user keypair:"
echo "   nak key generate"
echo ""
echo "2. Send NIP-04 encrypted message:"
echo "   nak event --sec <your-nsec> --kind 4 -p <service-pubkey-hex> --content '<json>' <relay-urls>"
echo ""
echo "3. Listen for NIP-04 responses:"
echo "   nak req --sec <your-nsec> --kind 4 --stream <relay-urls>"
echo ""
echo "4. Get recent direct messages:"
echo "   nak get-events --kind 4 --limit 10"
echo ""
echo "5. Decrypt specific message:"
echo "   nak get-event --event-id <event-id>"
echo ""
echo "🔧 Example NIP-04 Request JSON:"
echo "{\"action\": \"spawn_pod\", \"offer_id\": \"basic-ssh-pod\", \"duration_minutes\": 60, \"payment_proof\": \"cashu1...\"}"
echo ""
echo "✅ NIP-04 testing completed successfully!"