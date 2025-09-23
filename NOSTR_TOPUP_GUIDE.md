# Nostr Top-up Guide 🚀

## ✅ **Yes! Nostr Top-ups are Fully Supported**

The system supports both **HTTP mode** and **Nostr mode** for pod creation and top-ups. Here's how to use Nostr for extending pod duration.

## 🎯 **Nostr Event Types**

### **1. Pod Creation (Kind 1000)**
```json
{
  "kind": 1000,
  "content": "encrypted_content_here",
  "tags": [["encrypted"]]
}
```

### **2. Pod Top-up (Kind 1002)**
```json
{
  "kind": 1002,
  "content": "encrypted_content_here", 
  "tags": [["encrypted"]]
}
```

## 🔐 **Encrypted Content Structures**

### **Pod Creation Request (Kind 1000)**
```json
{
  "cashu_token": "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHBzOi8vbm9mZWVzLnRlc3RudXQuY2FzaHUuc3BhY2UiLCJwcm9vZnMiOlt7ImlkIjoi...",
  "ssh_username": "optional_username",
  "pod_image": "optional_image",
  "duration_minutes": 120
}
```

### **Pod Top-up Request (Kind 1002)**
```json
{
  "pod_name": "ssh-pod-abc12345",
  "cashu_token": "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHBzOi8vbm9mZWVzLnRlc3RudXQuY2FzaHUuc3BhY2UiLCJwcm9vZnMiOlt7ImlkIjoi..."
}
```

## 🛠️ **How to Send Nostr Events**

### **1. Using nostr-tools (Command Line)**
```bash
# Install nostr-tools
npm install -g nostr-tools

# Create pod (Kind 1000)
echo '{"cashu_token":"your_token_here","duration_minutes":120}' | \
nostr-tools encrypt --key your_nsec_key --pubkey service_npub_key | \
nostr-tools publish --relay wss://relay.damus.io --kind 1000

# Top-up pod (Kind 1002)  
echo '{"pod_name":"ssh-pod-abc12345","cashu_token":"your_topup_token"}' | \
nostr-tools encrypt --key your_nsec_key --pubkey service_npub_key | \
nostr-tools publish --relay wss://relay.damus.io --kind 1002
```

### **2. Using Python with nostr library**
```python
import asyncio
from nostr.key import PrivateKey
from nostr.relay import Relay
from nostr.event import Event
from nostr.nip04 import encrypt

async def create_pod():
    # Your keys
    user_private_key = PrivateKey.from_nsec("your_nsec_key")
    service_public_key = "service_npub_key"
    
    # Create pod request
    request = {
        "cashu_token": "your_token_here",
        "duration_minutes": 120,
        "ssh_username": "myuser"
    }
    
    # Encrypt the request
    encrypted_content = encrypt(user_private_key.hex(), service_public_key, json.dumps(request))
    
    # Create and send event
    event = Event(
        kind=1000,
        content=encrypted_content,
        tags=[["encrypted"]]
    )
    event.sign(user_private_key.hex())
    
    # Send to relay
    async with Relay("wss://relay.damus.io") as relay:
        await relay.publish(event)

async def topup_pod():
    # Top-up request
    request = {
        "pod_name": "ssh-pod-abc12345",
        "cashu_token": "your_topup_token"
    }
    
    # Encrypt and send (similar to above but kind=1002)
    encrypted_content = encrypt(user_private_key.hex(), service_public_key, json.dumps(request))
    
    event = Event(
        kind=1002,
        content=encrypted_content,
        tags=[["encrypted"]]
    )
    event.sign(user_private_key.hex())
    
    async with Relay("wss://relay.damus.io") as relay:
        await relay.publish(event)

# Run the functions
asyncio.run(create_pod())
asyncio.run(topup_pod())
```

### **3. Using JavaScript/TypeScript**
```javascript
import { Relay } from 'nostr-tools'
import { nip04 } from 'nostr-tools'

async function createPod() {
  const userPrivateKey = 'your_nsec_key'
  const servicePublicKey = 'service_npub_key'
  
  const request = {
    cashu_token: 'your_token_here',
    duration_minutes: 120,
    ssh_username: 'myuser'
  }
  
  // Encrypt the request
  const encryptedContent = await nip04.encrypt(userPrivateKey, servicePublicKey, JSON.stringify(request))
  
  // Create and send event
  const event = {
    kind: 1000,
    content: encryptedContent,
    tags: [['encrypted']],
    created_at: Math.floor(Date.now() / 1000)
  }
  
  // Sign and send
  const signedEvent = await window.nostr.signEvent(event)
  
  const relay = new Relay('wss://relay.damus.io')
  await relay.connect()
  await relay.publish(signedEvent)
}

async function topupPod() {
  const request = {
    pod_name: 'ssh-pod-abc12345',
    cashu_token: 'your_topup_token'
  }
  
  // Similar to above but kind=1002
  const encryptedContent = await nip04.encrypt(userPrivateKey, servicePublicKey, JSON.stringify(request))
  
  const event = {
    kind: 1002,
    content: encryptedContent,
    tags: [['encrypted']],
    created_at: Math.floor(Date.now() / 1000)
  }
  
  const signedEvent = await window.nostr.signEvent(event)
  await relay.publish(signedEvent)
}
```

## 🔄 **Complete Workflow**

### **1. Get Service Public Key**
```bash
# The service publishes its public key when it starts
# Look for events from the service with kind 20000 (offer events)
```

### **2. Create Pod via Nostr**
```bash
# Send encrypted pod creation request (kind 1000)
# Service will create pod and send back access details (kind 1001)
```

### **3. Extend Pod via Nostr**
```bash
# Send encrypted top-up request (kind 1002)
# Service will extend the pod's activeDeadlineSeconds
# No response needed - pod is extended automatically
```

## 📊 **Event Flow**

```
User                    Service
  |                       |
  |-- Kind 1000 --------->|  (Create pod request)
  |                       |-- Creates pod with activeDeadlineSeconds
  |<-- Kind 1001 ---------|  (Access details response)
  |                       |
  |-- Kind 1002 --------->|  (Top-up request)
  |                       |-- Extends activeDeadlineSeconds
  |                       |  (No response needed)
```

## 🎯 **Key Features**

### **✅ Encrypted Communication**
- All requests are encrypted using NIP-17 Gift Wrap
- Only the service can decrypt your requests
- Your private keys stay secure

### **✅ Automatic Pod Management**
- Pods created with `activeDeadlineSeconds`
- Top-ups extend the deadline automatically
- Kubernetes handles termination

### **✅ No HTTP Required**
- Pure Nostr protocol
- Works with any Nostr client
- Decentralized and censorship-resistant

### **✅ Same Functionality as HTTP**
- Create pods with custom duration
- Extend pod duration with top-ups
- All payment verification included

## 🚀 **Example Commands**

```bash
# 1. Create a 2-hour pod
echo '{"cashu_token":"token123","duration_minutes":120}' | \
nostr-tools encrypt --key nsec1... --pubkey npub1... | \
nostr-tools publish --relay wss://relay.damus.io --kind 1000

# 2. Extend the pod by 1 hour
echo '{"pod_name":"ssh-pod-abc12345","cashu_token":"topup456"}' | \
nostr-tools encrypt --key nsec1... --pubkey npub1... | \
nostr-tools publish --relay wss://relay.damus.io --kind 1002
```

## 🎉 **Result**

You can now **create and extend pods entirely through Nostr** - no HTTP endpoints needed! The system supports both modes seamlessly. 🚀
