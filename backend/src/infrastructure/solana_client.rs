use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::{Keypair, Signer}, transaction::Transaction
};
use solana_sdk::signature::Signature;
use anyhow::Result;
use std::sync::Arc;

pub struct SolanaClient {
    pub program_id: Pubkey,
    pub payer: Arc<Keypair>,
    pub rpc_url: String,
}

impl SolanaClient {
    pub fn new(
        program_id: Pubkey,
        payer: Arc<Keypair>,
        rpc_url: String,
    ) -> Self {
        Self {
            program_id,
            payer,
            rpc_url,
        }
    }
    
    pub fn new_devnet(program_id: Pubkey, payer: Arc<Keypair>) -> Self {
        Self::new(
            program_id,
            payer,
            "https://api.devnet.solana.com".to_string(),
        )
    }
    
    pub fn new_mainnet(program_id: Pubkey, payer: Arc<Keypair>) -> Self {
        Self::new(
            program_id,
            payer,
            "https://api.mainnet-beta.solana.com".to_string(),
        )
    }
    
    /// Derive user account PDA
    pub fn derive_user_account_pda(&self, owner: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"user", owner.as_ref()],
            &self.program_id,
        )
    }
    
    /// Derive position PDA
    pub fn derive_position_pda(
        &self,
        owner: &Pubkey,
        position_index: u32,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"position",
                owner.as_ref(),
                &position_index.to_le_bytes(),
            ],
            &self.program_id,
        )
    }
    
    /// Get payer pubkey
    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }
    /// Send transaction to Solana
    pub fn send_transaction(&self, instructions: &[Instruction]) -> Result<Signature> {
        let rpc_client = RpcClient::new(&self.rpc_url);
        
        // Get recent blockhash
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        
        // Create transaction
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&self.payer.pubkey()),
            &[&*self.payer],
            recent_blockhash,
        );
        
        // Send and confirm
        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
        
        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_derive_user_pda() {
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        let client = SolanaClient::new_devnet(program_id, payer.clone());
        
        let owner = payer.pubkey();
        let (pda, _bump) = client.derive_user_account_pda(&owner);
        
        // PDA should be deterministic
        let (pda2, _bump2) = client.derive_user_account_pda(&owner);
        assert_eq!(pda, pda2);
    }
    
    #[test]
    fn test_derive_position_pda() {
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        let client = SolanaClient::new_devnet(program_id, payer.clone());
        
        let owner = payer.pubkey();
        let (pda1, _) = client.derive_position_pda(&owner, 0);
        let (pda2, _) = client.derive_position_pda(&owner, 1);
        
        // Different indices should produce different PDAs
        assert_ne!(pda1, pda2);
    }
    
    #[test]
    fn test_create_devnet_client() {
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        let client = SolanaClient::new_devnet(program_id, payer);
        
        assert_eq!(client.rpc_url, "https://api.devnet.solana.com");
    }
    
    #[test]
    fn test_create_mainnet_client() {
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        let client = SolanaClient::new_mainnet(program_id, payer);
        
        assert_eq!(client.rpc_url, "https://api.mainnet-beta.solana.com");
    }
}
