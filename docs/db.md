# Database Schema - Perpetual Futures Backend

## Architecture Overview

The database layer stores all position data (current and historical) for efficient querying and analytics. Position updates are streamed via gRPC, pushed to Kafka topics, and inserted into PostgreSQL in real-time through MSK connectors. Haven't implemented the db layer due to some time constraints, but would be pretty straight forward.

Single table design storing all position states (current + historical). Each update creates a new row with higher `slot` and `timestamp`. The highest slot for a position represents the current state, while all rows provide complete audit trail.
Position updates streamed via

gRPC → Kafka → PostgreSQL via MSK connectors.

## Table Schema

### `positions` - All Position States (Current + Historical)

```
CREATE TABLE positions (
    id BIGSERIAL PRIMARY KEY,
    
    -- Solana identifiers
    position_account VARCHAR(44) NOT NULL,     -- Solana position PDA
    position_index INTEGER NOT NULL,
    owner VARCHAR(44) NOT NULL,                -- User wallet pubkey
    signature VARCHAR(88) NOT NULL,            -- Transaction signature
    slot BIGINT NOT NULL,                      -- Solana slot number
    block_time TIMESTAMP NOT NULL,             -- Block timestamp
    
    -- Position data
    symbol VARCHAR(20) NOT NULL,               -- "BTC-USD", "ETH-USD"
    side VARCHAR(10) NOT NULL,                 -- "Long" or "Short"
    size NUMERIC(38, 8) NOT NULL,              -- Position size
    entry_price NUMERIC(38, 6) NOT NULL,       -- Entry price
    mark_price NUMERIC(38, 6) NOT NULL,        -- Mark price at this update
    margin NUMERIC(38, 6) NOT NULL,            -- Margin
    leverage SMALLINT NOT NULL,                -- Leverage
    unrealized_pnl NUMERIC(38, 6) NOT NULL,    -- Unrealized PnL
    realized_pnl NUMERIC(38, 6) NOT NULL,      -- Realized PnL
    funding_accrued NUMERIC(38, 6) NOT NULL,   -- Funding
    liquidation_price NUMERIC(38, 6) NOT NULL, -- Liquidation price
    
    -- State tracking
    status VARCHAR(20) NOT NULL,               -- "Opening", "Open", "Modifying", "Closing", "Closed", "Liquidating"
    event_type VARCHAR(20) NOT NULL,           -- "OPENED", "MODIFIED", "CLOSED", "LIQUIDATED", "PNL_UPDATE"
    
    -- Metadata
    created_at TIMESTAMP DEFAULT NOW(),
    
    -- Indexes for efficient queries
    INDEX idx_position_slot (position_account, slot DESC),
    INDEX idx_owner_slot (owner, slot DESC),
    INDEX idx_symbol_slot (symbol, slot DESC),
    INDEX idx_signature (signature),
    INDEX idx_block_time (block_time DESC),
    INDEX idx_status (status, slot DESC),
    INDEX idx_liquidation (symbol, side, liquidation_price, slot DESC)
);

-- Unique constraint: one signature per position update
CREATE UNIQUE INDEX idx_unique_position_signature ON positions (position_account, signature);
```

## Views

### 1. Current Positions (Latest State)

```
CREATE VIEW current_positions AS
SELECT DISTINCT ON (position_account)
    position_account,
    position_index,
    owner,
    signature,
    slot,
    block_time,
    symbol,
    side,
    size,
    entry_price,
    mark_price,
    margin,
    leverage,
    unrealized_pnl,
    realized_pnl,
    funding_accrued,
    liquidation_price,
    status,
    event_type
FROM positions
ORDER BY position_account, slot DESC;

-- Index for view performance
CREATE INDEX idx_current_positions ON positions (position_account, slot DESC);
```

### 2. User Statistics (Aggregated)

```
CREATE VIEW user_statistics AS
SELECT 
    owner,
    COUNT(DISTINCT position_account) AS total_positions,
    COUNT(DISTINCT CASE WHEN status = 'Closed' THEN position_account END) AS closed_positions,
    COUNT(DISTINCT CASE WHEN status = 'Liquidating' THEN position_account END) AS liquidated_positions,
    SUM(CASE WHEN status = 'Closed' THEN realized_pnl ELSE 0 END) AS total_realized_pnl,
    SUM(CASE WHEN status IN ('Open', 'Opening', 'Modifying') THEN unrealized_pnl ELSE 0 END) AS total_unrealized_pnl,
    COUNT(DISTINCT CASE WHEN status = 'Closed' AND realized_pnl > 0 THEN position_account END) AS win_count,
    COUNT(DISTINCT CASE WHEN status = 'Closed' AND realized_pnl < 0 THEN position_account END) AS loss_count,
    MAX(block_time) AS last_activity
FROM current_positions
GROUP BY owner;
```

### 3. Daily PnL Snapshots

```
CREATE VIEW daily_pnl_snapshots AS
SELECT 
    owner,
    symbol,
    DATE_TRUNC('day', block_time) AS snapshot_day,
    SUM(unrealized_pnl) AS total_unrealized_pnl,
    SUM(realized_pnl) AS total_realized_pnl,
    SUM(funding_accrued) AS total_funding,
    COUNT(DISTINCT position_account) AS position_count
FROM positions
GROUP BY owner, symbol, DATE_TRUNC('day', block_time)
ORDER BY snapshot_day DESC;
```