use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_lock_ix(payer: &Keypair, config: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Lock {}.data(),
        amm_video::accounts::Lock {
            authority: payer.pubkey(),
            config,
        }
        .to_account_metas(None),
    )
}

pub fn create_unlock_ix(payer: &Keypair, config: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Unlock {}.data(),
        amm_video::accounts::Lock {
            authority: payer.pubkey(),
            config,
        }
        .to_account_metas(None),
    )
}
