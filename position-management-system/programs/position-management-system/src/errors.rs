use anchor_lang::prelude::*;

#[error_code]
pub enum PositionError {
    #[msg("Leverage exceeds maximum allowed for position size")]
    LeverageExceeded,
    
    #[msg("Position size exceeds tier limit")]
    PositionSizeTooLarge,
    
    #[msg("Insufficient collateral for position")]
    InsufficientCollateral,
    
    #[msg("Leverage must be between 1 and 1000")]
    InvalidLeverage,
    
    #[msg("Position size must be greater than 0")]
    InvalidPositionSize,
    
    #[msg("Margin ratio too low, position at risk")]
    MarginRatioTooLow,
    
    #[msg("Cannot remove margin, would cause liquidation")]
    CannotRemoveMargin,
    
    #[msg("Invalid symbol")]
    InvalidSymbol,
    
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[msg("Position is not open")]
    PositionNotOpen,
    
    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Invalid or stale oracle price")]
    InvalidPrice,
    
    #[msg("Price slippage exceeded maximum")]
    SlippageExceeded,
    
    #[msg("Invalid slippage parameter")]
    InvalidSlippage,
}
