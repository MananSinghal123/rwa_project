use anchor_lang::prelude::*;

declare_id!("u2LHcL4X3qhrJfmzkhinuPqEcctJPyosegum6Tdi5Nk");

#[program]
pub mod program {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
