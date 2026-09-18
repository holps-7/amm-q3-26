use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    litesvm::LiteSVM,
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_initialise_ix(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
    mint_lp: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
) -> Instruction {
    create_initialise_ix_with_fees(
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, 30, 100,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_initialise_ix_with_fees(
    mut _svm: &mut LiteSVM,
    payer: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
    mint_lp: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
    fee: u16,
    protocol_fee: u16,
) -> Instruction {
    let maker = payer.pubkey();
    let treasury_x =
        Pubkey::find_program_address(&[b"treasury_x", config.as_ref()], &amm_video::id()).0;
    let treasury_y =
        Pubkey::find_program_address(&[b"treasury_y", config.as_ref()], &amm_video::id()).0;

    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Initialize {
            seed: 123,
            fee,
            protocol_fee,
            authority: Some(maker),
        }
        .data(),
        amm_video::accounts::Initialize {
            initializer: maker,
            mint_x,
            mint_y,
            mint_lp,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            config,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
