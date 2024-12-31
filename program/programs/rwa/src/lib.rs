use anchor_lang::{prelude::*,system_program::{create_account,CreateAccount}};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint,TokenAccount,TokenInterface},
};
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList,
};
use spl_transfer_hook_interface::instruction::{ExecuteInstruction, TransferHookInstruction};


declare_id!("u2LHcL4X3qhrJfmzkhinuPqEcctJPyosegum6Tdi5Nk");

#[account]
pub struct AssetDetails {
    pub asset_type: String,        // e.g., "RealEstate", "Art", "Vehicle"
    pub identifier: String,        // Legal identifier (deed number, registration, etc.)
    pub jurisdiction: String,      // Legal jurisdiction
    pub valuation: u64,           // Current valuation in smallest denomination
    pub last_audit_date: i64,     // Unix timestamp
    pub custodian: Pubkey,        // Entity responsible for asset custody
    pub compliance_status: bool,   // Whether asset is compliant
    pub metadata_uri: String,      // URI for additional metadata
}


#[program]
pub mod rwa {
    use super::*;

    pub fn initialize_asset(
        ctx: Context<InitializeAsset>,
        asset_type: String,
        identifier: String,
        jurisdiction: String,
        valuation: u64,
        metadata_uri: String,
    ) -> Result<()> {
        let asset_details: &mut Account<'_, AssetDetails> = &mut ctx.accounts.asset_details;
        asset_details.asset_type = asset_type;
        asset_details.identifier = identifier;
        asset_details.jurisdiction = jurisdiction;
        asset_details.valuation = valuation;
        asset_details.last_audit_date = Clock::get()?.unix_timestamp;
        asset_details.custodian = ctx.accounts.custodian.key();
        asset_details.compliance_status = true;
        asset_details.metadata_uri = metadata_uri;
        
        // Initialize the extra account meta list (same as before)
        let account_metas = vec![];
        let account_size: u64 = ExtraAccountMetaList::size_of(account_metas.len())? as u64;
        let lamports = Rent::get()?.minimum_balance(account_size as usize);
        
        let mint = ctx.accounts.mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"extra-account-metas",
            &mint.as_ref(),
            &[ctx.bumps.extra_account_meta_list],
        ]];

        create_account(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.extra_account_meta_list.to_account_info(),
                },
            )
            .with_signer(signer_seeds),
            lamports,
            account_size,
            ctx.program_id,
        )?;

        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
            &account_metas,
        )?;
        Ok(())
    }

    pub fn update_asset_details(
        ctx: Context<UpdateAssetDetails>,
        valuation: Option<u64>,
        compliance_status: Option<bool>,
        metadata_uri: Option<String>,
    ) -> Result<()> {
        let asset_details = &mut ctx.accounts.asset_details;
        
        require!(
            ctx.accounts.custodian.key() == asset_details.custodian,
            RwaError::UnauthorizedCustodian
        );

        if let Some(val) = valuation {
            asset_details.valuation = val;
        }
        if let Some(status) = compliance_status {
            asset_details.compliance_status = status;
        }
        if let Some(uri) = metadata_uri {
            asset_details.metadata_uri = uri;
        }
        
        Ok(())
    }


    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        let account_metas = vec![];
        // calculate account size
        let account_size: u64 = ExtraAccountMetaList::size_of(account_metas.len())? as u64;
        // calculate minimum required lamports
        let lamports = Rent::get()?.minimum_balance(account_size as usize);
 
        let mint = ctx.accounts.mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"extra-account-metas",
            &mint.as_ref(),
            &[ctx.bumps.extra_account_meta_list],
        ]];
 
        // create ExtraAccountMetaList account
        create_account(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.extra_account_meta_list.to_account_info(),
                },
            )
            .with_signer(signer_seeds),
            lamports,
            account_size,
            ctx.program_id,
        )?;
 
        // initialize ExtraAccountMetaList account with extra accounts
        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
            &account_metas,
        )?;
        Ok(())
    }

    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        // Get asset details
        let asset_details: &mut Account<'_, AssetDetails> = &mut ctx.accounts.asset_details;
        
        // Check compliance status
        require!(
            asset_details.compliance_status,
            RwaError::NonCompliantAsset
        );
        
        // Additional transfer checks can be added here
        // For example, checking transfer restrictions based on jurisdiction
        
        msg!("RWA Transfer Hook - Asset type: {}", asset_details.asset_type);
        msg!("Transfer amount: {}", amount);
        
        Ok(())
    }

    // fallback instruction handler as workaround to anchor instruction discriminator check
    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        let instruction = TransferHookInstruction::unpack(data)?;
    
        // match instruction discriminator to transfer hook interface execute instruction
        // token2022 program CPIs this instruction on token transfer
        match instruction {
            TransferHookInstruction::Execute { amount } => {
                let amount_bytes = amount.to_le_bytes();
    
                // invoke custom transfer hook instruction on our program
                __private::__global::transfer_hook(program_id, accounts, &amount_bytes)
            }
            _ => return Err(ProgramError::InvalidInstructionData.into()),
        }
    }

    }


#[derive(Accounts)]
pub struct InitializeAsset<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 32 + 32 + 8 + 8 + 32 + 1 + 200  // Adjust space as needed
    )]
    pub asset_details: Account<'info, AssetDetails>,
    
    pub custodian: Signer<'info>,
    
    /// CHECK: ExtraAccountMetaList Account
    #[account(
        mut,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct UpdateAssetDetails<'info> {
    pub custodian: Signer<'info>,
    #[account(mut)]
    pub asset_details: Account<'info, AssetDetails>,
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList Account, must use these seeds
    #[account(
        mut,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
 

#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account,
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    #[account(
        seeds = [b"asset", mint.key().as_ref()],
        bump,
    )]
    pub asset_details: Account<'info, AssetDetails>,
}

#[error_code]
pub enum RwaError {
    #[msg("Unauthorized custodian")]
    UnauthorizedCustodian,
    #[msg("Asset is not compliant")]
    NonCompliantAsset,
}




