use anchor_lang::prelude::*;
use anchor_lang::{AnchorDeserialize, AnchorSerialize};
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use std::io;
use std::io::Read;
use wormhole_anchor_sdk::wormhole;

declare_id!("HUeE8rXu3hW8Kbs6So6oe1zGL3DMi7zNckSFHm2UyYpj");

#[program]
pub mod airdrop {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info().clone(),
                Transfer {
                    authority: ctx.accounts.user.to_account_info().clone(),
                    from: ctx.accounts.source_account.to_account_info().clone(),
                    to: ctx.accounts.destination_account.to_account_info().clone(),
                },
            ),
            amount,
        )?;

        Ok(())
    }

    pub fn claim(
        ctx: Context<Claim>,
        claim_bump: u8,
        nft_eth_address: [u8; 20],
        nft_id: u16,
        vaa_hash: [u8; 32],
    ) -> Result<()> {
        let posted_message = &ctx.accounts.posted;

        if let AirdropMessage { account, nft, id } = posted_message.data() {
            require!(
                Pubkey::new_from_array(account.clone())
                    == ctx.accounts.user.to_account_info().key(),
                AirdropError::VerificationFailed
            );

            let claim_status = &mut ctx.accounts.claim_status;

            require!(
                // This check is redudant, we should not be able to initialize a claim status account at the same key.
                !claim_status.is_claimed && nft_eth_address == nft.clone() && nft_id == id.clone(),
                AirdropError::DropAlreadyClaimed
            );

            claim_status.is_claimed = true;
            claim_status.nft_eth_address = nft.clone();
            claim_status.nft_id = id.clone();

            let seeds = &[b"destination".as_ref(), &[claim_bump]];

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.from.to_account_info(),
                        to: ctx.accounts.to.to_account_info(),
                        authority: ctx.accounts.from.to_account_info(),
                    },
                )
                .with_signer(&[&seeds[..]]),
                100,
            )?;

            Ok(())
        } else {
            Err(AirdropError::InvalidMessage.into())
        }
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, seeds = [b"destination".as_ref()], bump, payer = user, space = TokenAccount::LEN)]
    pub destination_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(Default)]
pub struct ClaimStatus {
    /// If true, the tokens have been claimed.
    pub is_claimed: bool,
    /// Authority that claimed the tokens.
    pub nft_eth_address: [u8; 20], // Ethereum address of the NFT collection.
    pub nft_id: u16, // Identifier of the NFT in the collection.
}

#[derive(Accounts)]
#[instruction(nft_eth_address: [u8; 20], nft_id: u16, vaa_hash: [u8; 32])]
pub struct Claim<'info> {
    // Wormhole program.
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        seeds = [
            wormhole::SEED_PREFIX_POSTED_VAA,
            &vaa_hash
        ],
        bump,
        seeds::program = wormhole_program
    )]
    /// Verified Wormhole message account. The Wormhole program verified
    /// signatures and posted the account data here. Read-only.
    pub posted: Account<'info, wormhole::PostedVaa<AirdropMessage>>,

    #[account(
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            &posted.emitter_chain().to_le_bytes()[..]
        ],
        bump,
        constraint = foreign_emitter.verify(posted.emitter_address()) @ AirdropError::InvalidForeignEmitter
    )]
    /// Foreign emitter account. The posted message's `emitter_address` must
    /// agree with the one we have registered for this message's `emitter_chain`
    /// (chain ID). Read-only.
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    #[account(init, seeds=[b"claim".as_ref(), nft_eth_address.as_ref(), nft_id.to_be_bytes().as_ref()], space = 1 + 160 + 16 + 8, bump, payer = user)]
    pub claim_status: Account<'info, ClaimStatus>,

    #[account(mut, seeds = [b"destination".as_ref()], bump)]
    pub from: Account<'info, TokenAccount>,
    /// Account to send the claimed tokens to.
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// SPL [Token] program.
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub source_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"destination".as_ref()], bump)]
    pub destination_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
/// Foreign emitter account data.
pub struct ForeignEmitter {
    /// Emitter chain. Cannot equal `1` (Solana's Chain ID).
    pub chain: u16,
    /// Emitter address. Cannot be zero address.
    pub address: [u8; 32],
}

impl ForeignEmitter {
    pub const MAXIMUM_SIZE: usize = 8 // discriminator
        + 2 // chain
        + 32 // address
    ;
    /// AKA `b"foreign_emitter"`.
    pub const SEED_PREFIX: &'static [u8; 15] = b"foreign_emitter";

    /// Convenience method to check whether an address equals the one saved in
    /// this account.
    pub fn verify(&self, address: &[u8; 32]) -> bool {
        *address == self.address
    }
}

#[derive(Clone)]
pub struct AirdropMessage {
    account: [u8; 32],
    nft: [u8; 20],
    id: u16,
}

impl AnchorDeserialize for AirdropMessage {
    fn deserialize_reader<R: Read>(buf: &mut R) -> io::Result<Self> {
        let mut buffer = [0u8; 54];
        buf.read_exact(&mut buffer)?;

        // Prepare arrays to hold the account and nft data.
        let mut account = [0u8; 32];
        let mut nft = [0u8; 20];

        // Copy data from buffer into the arrays.
        account.copy_from_slice(&buffer[0..32]);
        nft.copy_from_slice(&buffer[32..52]);

        // Extract the id directly from the buffer as before.
        let id = u16::from_be_bytes([buffer[52], buffer[53]]);

        Ok(AirdropMessage { account, nft, id })
    }
}

impl AnchorSerialize for AirdropMessage {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let AirdropMessage { account, nft, id } = self;
        account.serialize(writer)?;
        nft.serialize(writer)?;
        id.to_be_bytes().serialize(writer)?;
        Ok(())
    }
}

#[error_code]
pub enum AirdropError {
    #[msg("Invalid Merkle proof.")]
    InvalidProof,
    #[msg("Drop already claimed.")]
    DropAlreadyClaimed,
    #[msg("Account is not authorized to execute this instruction")]
    Unauthorized,
    #[msg("Token account owner did not match intended owner")]
    OwnerMismatch,
    #[msg("Temporal signer did not match distributor")]
    TemporalMismatch,
    #[msg("Numerical Overflow")]
    NumericalOverflow,
    #[msg("Invalid Claim Bump")]
    InvalidClaimBump,
    #[msg("Airdrop only supports the official Metaplex Candy machine contracts")]
    MustUseOfficialCandyMachine,
    #[msg("Bump seed not in hash map")]
    BumpSeedNotInHashMap,
    #[msg("InvalidForeignEmitter")]
    InvalidForeignEmitter,
    #[msg("InvalidMessage")]
    InvalidMessage,
    #[msg("VerificationFailed")]
    VerificationFailed,
}
