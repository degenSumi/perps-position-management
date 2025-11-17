# **API Specifications - Perpetual Futures Backend**

Version: 1.0.0  
Base URL: `http://localhost:3000` (development)

***

## **Table of Contents**

1. [Authentication](#authentication)
2. [User Management](#user-management)
3. [Position Management](#position-management)
4. [Monitoring & Analytics](#monitoring--analytics)
5. [WebSocket Streams](#websocket-streams)
6. [Error Handling](#error-handling)

***

## **Authentication**

Currently, authentication is handled via Solana wallet signatures. All transactions require the user's keypair to sign on-chain operations.

***

## **User Management**

### **Initialize User Account**

Create a new user account on-chain.

**Endpoint:** `POST /users/initialize`

**Request Body:**
```json
{
  "owner": "string"  // Solana wallet public key (base58)
}
```

**Response:** `200 OK`
```json
{
  "signature": "string",  // Transaction signature
  "message": "User account initialized"
}
```

**Example:**
```bash
curl -X POST http://localhost:3000/users/initialize \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "" // pubkey of the account added in the env
  }'
```

***

### **Add Collateral**

Add collateral to a user's account.

**Endpoint:** `POST /users/:owner/collateral`

**Path Parameters:**
- `owner` - User's Solana public key (base58)

**Request Body:**
```json
{
  "amount": "number"  // Amount in smallest units (e.g., lamports for SOL)
}
```

**Response:** `200 OK`
```json
{
  "signature": "string",
  "message": "Added {amount} collateral"
}
```

**Example:**
```bash
curl -X POST http://localhost:3000/users/6z6EVx9ZVbHkZ3SmNuCmDEYvQmyUiQ2GtJxJ7KFQoYpz/collateral \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 1000000000000
  }'
```

***

### **Get User Account**

Retrieve user account details from on-chain.

**Endpoint:** `GET /users/:owner/account`

**Path Parameters:**
- `owner` - User's Solana public key (base58)

**Response:** `200 OK`
```json
{
  "owner": "string",
  "total_collateral": "number",
  "locked_collateral": "number",
  "available_collateral": "number",
  "total_pnl": "number",
  "position_count": "number",
  "position_count_total": "number"
}
```

**Example:**
```bash
curl http://localhost:3000/users/6z6EVx9ZVbHkZ3SmNuCmDEYvQmyUiQ2GtJxJ7KFQoYpz/account
```

***

### **Get User's Positions**

Retrieve all positions for a user.

**Endpoint:** `GET /users/:owner/positions`

**Path Parameters:**
- `owner` - User's Solana public key (base58)

**Response:** `200 OK`
```json
[
  {
    "position_account": "string",
    "position_index": "number",
    "owner": "string",
    "symbol": "string",
    "side": "Long" | "Short",
    "size": "string",
    "entry_price": "string",
    "mark_price": "string",
    "margin": "string",
    "leverage": "number",
    "unrealized_pnl": "string",
    "realized_pnl": "string",
    "liquidation_price": "string",
    "status": "string",
    "opened_at": "string",
    "last_update": "string"
  }
]
```

**Example:**
```bash
curl http://localhost:3000/users/6z6EVx9ZVbHkZ3SmNuCmDEYvQmyUiQ2GtJxJ7KFQoYpz/positions
```

***

## **Position Management**

### **Open Position**

Open a new leveraged position.

**Endpoint:** `POST /positions/open`

**Request Body:**
```json
{
  "owner": "string",               // Solana wallet public key
  "symbol": "string",              // Trading pair (e.g., "BTC-USD")
  "side": "Long" | "Short",        // Position side
  "size": "string",                // Position size (decimal string)
  "leverage": "number",            // Leverage multiplier (1-100)
  "entry_price": "string",         // Entry price (decimal string)
  "maintenance_margin_ratio": "string"  // Optional, defaults to 0.025 (2.5%)
}
```

**Response:** `200 OK`
```json
{
  "position": {
    "position_account": "string",
    "position_index": "number",
    "owner": "string",
    "symbol": "string",
    "side": "Long" | "Short",
    "size": "string",
    "entry_price": "string",
    "mark_price": "string",
    "margin": "string",
    "leverage": "number",
    "unrealized_pnl": "string",
    "realized_pnl": "string",
    "liquidation_price": "string",
    "status": "string",
    "opened_at": "string",
    "last_update": "string"
  },
  "signature": "string"
}
```

**Example:**
```bash
curl -X POST http://localhost:3000/positions/open \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "6z6EVx9ZVbHkZ3SmNuCmDEYvQmyUiQ2GtJxJ7KFQoYpz",
    "symbol": "BTC-USD",
    "side": "Long",
    "size": "0.1",
    "leverage": 10,
    "entry_price": "95000"
  }'
```

***

### **Modify Position**

Modify an existing position's size or margin.

**Endpoint:** `PUT /positions/:position_account/modify`

**Path Parameters:**
- `position_account` - Position's Solana account address (base58)

**Request Body:**
```json
{
  "new_size": "string" | null,      // New position size (optional)
  "margin_delta": "number" | null   // Margin adjustment (optional)
}
```

**Response:** `200 OK`
```json
{
  "signature": "string",
  "message": "Position modified successfully"
}
```

**Example:**
```bash
curl -X PUT http://localhost:3000/positions/8Kp2Lm9nT5vR3xQ7yF1wD4cE6hN8sJ0mP2bV5gA9tU3/modify \
  -H "Content-Type: application/json" \
  -d '{
    "new_size": "0.15",
    "margin_delta": null
  }'
```

***

### **Close Position**

Close an existing position at current market price.

**Endpoint:** `DELETE /positions/:position_account/close`

**Path Parameters:**
- `position_account` - Position's Solana account address (base58)

**Request Body:**
```json
{
  "final_price": "string"  // Closing price (decimal string)
}
```

**Response:** `200 OK`
```json
{
  "pnl": "string",        // Total realized PnL
  "signature": "string",
  "message": "Position closed successfully"
}
```

**Example:**
```bash
curl -X DELETE http://localhost:3000/positions/3RNnmWpouF7UDetKCVRnZsSRmnpXyMTbMBb54zkN2eBB/close \
  -H "Content-Type: application/json" \
  -d '{
    "final_price": "96500"
  }'
```

***

### **Get Position Details**

Retrieve detailed information about a specific position.

**Endpoint:** `GET /positions/:position_account`

**Path Parameters:**
- `position_account` - Position's Solana account address (base58)

**Response:** `200 OK`
```json
{
  "position_account": "string",
  "position_index": "number",
  "owner": "string",
  "symbol": "string",
  "side": "Long" | "Short",
  "size": "string",
  "entry_price": "string",
  "mark_price": "string",
  "margin": "string",
  "leverage": "number",
  "unrealized_pnl": "string",
  "realized_pnl": "string",
  "liquidation_price": "string",
  "status": "string",
  "opened_at": "string",
  "last_update": "string"
}
```

**Example:**
```bash
curl http://localhost:3000/positions/3RNnmWpouF7UDetKCVRnZsSRmnpXyMTbMBb54zkN2eBB
```

***

## **Monitoring & Analytics**

### **Health Check**

Check if the API is running.

**Endpoint:** `GET /health`

**Response:** `200 OK`
```json
{
  "status": "healthy",
  "timestamp": "string"
}
```

**Example:**
```bash
curl http://localhost:3000/health
```

***

### **List All Positions**

Retrieve all monitored positions.

**Endpoint:** `GET /positions`

**Query Parameters:**
- `status` - Filter by status (optional): `Opening`, `Open`, `Modifying`, `Closing`, `Closed`
- `symbol` - Filter by trading pair (optional)

**Response:** `200 OK`
```json
[
  {
    "position_account": "string",
    "position_index": "number",
    "owner": "string",
    "symbol": "string",
    "side": "Long" | "Short",
    "size": "string",
    "entry_price": "string",
    "mark_price": "string",
    "unrealized_pnl": "string",
    "status": "string"
  }
]
```

**Example:**
```bash
curl http://localhost:3000/positions?status=Open&symbol=BTC-USD
```

***

### **Get Statistics**

Retrieve system-wide statistics.

**Endpoint:** `GET /statistics`

**Response:** `200 OK`
```json
{
  "total_positions": "number",
  "open_positions": "number",
  "assets_monitored": "number",
  "total_unrealized_pnl": "string"
}
```

**Example:**
```bash
curl http://localhost:3000/statistics
```

***

### **Get Prices**

Retrieve current prices for all monitored assets.

**Endpoint:** `GET /prices`

**Response:** `200 OK`
```json
{
  "BTC-USD": {
    "price": "string",
    "timestamp": "string"
  },
  "ETH-USD": {
    "price": "string",
    "timestamp": "string"
  }
}
```

**Example:**
```bash
curl http://localhost:3000/prices
```

***

### **Get Price for Symbol**

Retrieve current price for a specific asset.

**Endpoint:** `GET /prices/:symbol`

**Path Parameters:**
- `symbol` - Trading pair (e.g., "BTC-USD")

**Response:** `200 OK`
```json
{
  "symbol": "string",
  "price": "string",
  "timestamp": "string"
}
```

**Example:**
```bash
curl http://localhost:3000/prices/BTC-USD
```

***

## **WebSocket Streams**

### **Connect to WebSocket**

Establish a WebSocket connection for real-time updates.

**Endpoint:** `ws://localhost:3000/ws`

**Connection:**
```javascript
const ws = new WebSocket('ws://localhost:3000/ws');
```

***

### **Subscribe to Symbol**

Subscribe to updates for a specific trading pair.

**Message:**
```json
{
  "type": "subscribe_symbol",
  "symbol": "BTC-USD"
}
```

**Example:**
```javascript
ws.send(JSON.stringify({
  type: "subscribe_symbol",
  symbol: "BTC-USD"
}));
```

***

### **Unsubscribe from Symbol**

Unsubscribe from updates for a trading pair.

**Message:**
```json
{
  "type": "unsubscribe_symbol",
  "symbol": "BTC-USD"
}
```

**Example:**
```javascript
ws.send(JSON.stringify({
  type: "unsubscribe_symbol",
  symbol: "BTC-USD"
}));
```

***

### **Message Types**

#### **Connected**
Sent when WebSocket connection is established.

```json
{
  "type": "connected",
  "message": "Connected to Perpetual Futures Backend"
}
```

***

#### **Price Update**
Real-time price updates for subscribed symbols.

```json
{
  "type": "price_update",
  "symbol": "BTC-USD",
  "price": "95000.50",
  "timestamp": "2025-11-17T15:30:00Z"
}
```

***

#### **Position Update**
Real-time position PnL and state updates.

```json
{
  "type": "position_update",
  "position_account": "string",
  "symbol": "BTC-USD",
  "side": "Long",
  "size": "0.1",
  "entry_price": "94000.00",
  "mark_price": "95000.50",
  "unrealized_pnl": "150.05",
  "margin_ratio": "0.15",
  "timestamp": "2025-11-17T15:30:01Z"
}
```

***

#### **Liquidation Alert**
Alerts when positions are near liquidation.

```json
{
  "type": "liquidation_alert",
  "position_account": "string",
  "symbol": "BTC-USD",
  "side": "Long",
  "liquidation_price": "85000.00",
  "current_price": "85500.00"
}
```

***

#### **Error**
Error messages for invalid commands.

```json
{
  "type": "error",
  "message": "Invalid command: ..."
}
```

***

## **Error Handling**

### **Error Response Format**

All errors follow this format:

```json
{
  "error": "string",
  "message": "string",
  "details": "string"  // Optional
}
```

***

### **HTTP Status Codes**

| Code | Description |
|------|-------------|
| `200` | Success |
| `201` | Created |
| `400` | Bad Request - Invalid parameters |
| `404` | Not Found - Resource doesn't exist |
| `500` | Internal Server Error |
| `503` | Service Unavailable |

***

### **Common Errors**

#### **Invalid Pubkey**
```json
{
  "error": "BadRequest",
  "message": "Invalid owner pubkey: ..."
}
```

#### **Position Not Found**
```json
{
  "error": "NotFound",
  "message": "Position not found"
}
```

#### **Transaction Failed**
```json
{
  "error": "InternalError",
  "message": "Failed to send transaction: ..."
}
```

***

## **Rate Limiting**

Currently no rate limiting is enforced. Production deployments should implement rate limiting per IP/user.

***

## **Data Types**

### **Decimal Precision**

- **Prices (USDT)**: 6 decimal places
- **Sizes (BTC/ETH)**: 8 decimal places
- **PnL**: 6 decimal places

### **Timestamp Format**

ISO 8601: `2025-11-17T15:30:00Z`

***

## **Notes**

- All numeric values in responses are returned as strings to preserve precision
- Solana addresses are base58-encoded strings
- Transaction signatures are base58-encoded strings
- WebSocket subscriptions default to "all symbols" if none specified

***

**End of API Specifications**

Sources
