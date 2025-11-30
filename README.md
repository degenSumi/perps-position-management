<!-- High level architecture diagram -->
# **Architecture**
![High Level Architecture (Perpetual Futures Platform)](/docs/perps-architecture.png)

# **Build & Deployment Guide - Perpetual Futures Platform**

Complete guide for building and deploying the Solana smart contracts and Rust backend.

***

## **Table of Contents**

1. [Prerequisites](#prerequisites)
2. [Smart Contract Deployment](#smart-contract-deployment)
- **devnet deployment** https://solscan.io/account/9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3?cluster=devnet
3. [Backend Setup](#backend-setup)
7. [Monitoring & Maintenance](#monitoring--maintenance)

***

## **Prerequisites**

### **Required Tools**

- **Rust**: 1.70+ ([Install](https://rustup.rs/))
- **Solana CLI**: 1.17+ ([Install](https://docs.solana.com/cli/install-solana-cli-tools))
- **Anchor Framework**: 0.29+ ([Install](https://www.anchor-lang.com/docs/installation))
- **Node.js**: 18+ ([Install](https://nodejs.org/))
- **PostgreSQL**: 14+ ([Install](https://www.postgresql.org/download/))
- **Redis**: 7+ ([Install](https://redis.io/download))

### **Install Rust**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### **Install Solana CLI**

```bash
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"
export PATH="/home/$USER/.local/share/solana/install/active_release/bin:$PATH"
```

### **Install Anchor**

```bash
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest
```

***

## **Smart Contract Deployment**

### **1. Clone Repository**

```bash
git clone git@github.com:degenSumi/perps-position-management.git
cd perps-position-management
```

### **2. Navigate to Contract Directory**

```bash
cd position-management-system
```

### **3. Configure Solana CLI**

#### **For Devnet:**

```bash
solana config set --url https://api.devnet.solana.com
```

#### **For Mainnet:**

```bash
solana config set --url https://api.mainnet-beta.solana.com
```

### **4. Create Wallet (if needed)**

```bash
solana-keygen new --outfile ~/.config/solana/deployer.json
```

### **5. Airdrop SOL (Devnet only)**

```bash
solana airdrop 2
```

### **6. Build Contract**

```bash
anchor build
```

This generates:
- Program binary: `target/deploy/position_management_system.so`
- IDL: `target/idl/position_management_system.json`

### **7. Get Program ID**

```bash
anchor keys list
```

Copy the program ID and update it in:
- `Anchor.toml`
- `programs/position_management_system/src/lib.rs` (at `declare_id!`)

### **8. Rebuild After Program ID Update**

```bash
anchor build
```

### **9. Deploy to Devnet**

```bash
anchor deploy
```

### **10. Verify Deployment**

```bash
solana program show 
```

### **12. Run Tests**

```bash
anchor test
```

***

## **Backend Setup**

### **1. Navigate to Backend Directory**

```bash
cd backend
```

### **2. Configure Environment**

Create `.env` file:
Or use the dev env

```bash
# Solana Configuration
SOLANA_RPC_URL=https://api.devnet.solana.com
PROGRAM_ID=9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3
SOLANA_PRIVATE_KEY=<BASE58_PRIVATE_KEY> # imp used for all the transactions

# Redis Configuration
REDIS_URL=redis://localhost:6379

# Server Configuration
PORT=3000


# Monitoring
RUST_LOG=info
```

### **3. Install Dependencies**

```bash
cargo build
```

### **4. Start Backend (Development)**

```bash
cargo run
```

### **6. Build for Production**

```bash
cargo build --release
```

## **Post-Deployment Checklist**

- [ ] Smart contracts deployed and verified
- [ ] Backend running and accessible
- [ ] Database initialized with schema
- [ ] Redis connected
- [ ] WebSocket connections working
- [ ] Health check endpoint responds
- [ ] Position opening/closing tested
- [ ] Liquidation alerts working
- [ ] Logs properly configured

***

## **Troubleshooting**

### **Contract Deployment Fails**

- Check SOL balance: `solana balance`
- Verify RPC URL: `solana config get`
- Check program size: `ls -lh target/deploy/*.so`

### **Backend Won't Start**

- Check `.env` configuration
- Check Redis connectivity
- Review logs for errors

### **WebSocket Not Working**

- Check firewall rules
- Verify port 3000 is open
- Test with Postman or wscat

***

**End of Deployment Guide**
