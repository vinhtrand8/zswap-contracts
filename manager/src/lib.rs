use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap};
use near_sdk::json_types::{I128, U128};
use near_sdk::{
    env, log, near_bindgen, serde_json, AccountId, BorshStorageKey, CryptoHash, PanicOnDefault,
    Promise, PromiseError,
};
use pool::{ext_zswap_pool, Slot0};

use crate::utils::*;

mod callback;
mod error;
pub mod ft_receiver;
mod internal;
mod nft;
mod pool;
pub mod utils;

// Define the contract structure
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    factory: AccountId,
    token_id: u128,
    nft: NonFungibleToken,
    metadata: LazyOption<NFTContractMetadata>,
    account_tokens: LookupMap<CryptoHash, u128>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    NonFungibleToken,
    Metadata,
    TokenMetadata,
    Enumeration,
    Approval,
    AccountTokens,
}

// Implement the contract structure
#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(factory: AccountId) -> Self {
        let nft = NonFungibleToken::new(
            StorageKey::NonFungibleToken,
            env::current_account_id(),
            Some(StorageKey::TokenMetadata),
            Some(StorageKey::Enumeration),
            Some(StorageKey::Approval),
        );
        let metadata = NFTContractMetadata {
            spec: NFT_METADATA_SPEC.to_string(),
            name: "ZSwap Liquidity Management".to_string(),
            symbol: "ZLM".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        };
        Self {
            factory,
            token_id: 0,
            nft,
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
            account_tokens: LookupMap::new(StorageKey::AccountTokens),
        }
    }

    #[allow(unused)]
    pub fn get_position(
        &self,
        token_1: AccountId,
        token_2: AccountId,
        fee: u32,
        owner: AccountId,
        lower_tick: i32,
        upper_tick: i32,
    ) {
    }

    #[payable]
    pub fn mint(&mut self, params: MintParams) -> Promise {
        let pool = self.internal_get_pool(&params.token_0, &params.token_1, params.fee);
        let recipient = env::predecessor_account_id();

        ext_zswap_pool::ext(pool.clone()).get_slot_0().then(
            Self::ext(env::current_account_id())
                .with_attached_deposit(env::attached_deposit())
                .mint_calculate_liquidity(pool, recipient, params),
        )
    }

    #[payable]
    #[private]
    pub fn mint_calculate_liquidity(
        &mut self,
        #[callback_result] slot_0_res: Result<Slot0, PromiseError>,
        pool: AccountId,
        recipient: AccountId,
        params: MintParams,
    ) -> Promise {
        let slot_0 = slot_0_res.unwrap();

        let liquidity = self.internal_calculate_liquidity(
            slot_0,
            params.lower_tick,
            params.upper_tick,
            params.amount_0_desired.0,
            params.amount_1_desired.0,
        );
        log!("Liquidity: {}", liquidity);

        // mint nft
        let liquidity_info = NftLiquidityInfo {
            token_0: params.token_0.clone(),
            token_1: params.token_1.clone(),
            fee: params.fee,
            lower_tick: params.lower_tick,
            upper_tick: params.upper_tick,
            liquidity,
        };
        let nft_description = format!("ZSwap Liquidity NFT for {}", &pool);

        let liquidity_nft_metadata = TokenMetadata {
            title: Some("ZSwap Liquidity NFT".to_string()),
            description: Some(nft_description),
            media: None,
            media_hash: None,
            copies: None,
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: Some(serde_json::to_string(&liquidity_info).unwrap()),
            reference: None,
            reference_hash: None,
        };

        self.nft.internal_mint(
            self.token_id.to_string(),
            recipient.clone(),
            Some(liquidity_nft_metadata),
        );
        self.token_id += 1;

        ext_zswap_pool::ext(pool)
            .mint(
                recipient,
                params.lower_tick,
                params.upper_tick,
                U128::from(liquidity),
            )
            .then(
                Self::ext(env::current_account_id())
                    .mint_callback(params.amount_0_min.into(), params.amount_1_min.into()),
            )
    }

    #[private]
    pub fn calculate_amount_out(
        &mut self,
        #[callback_result] amounts_res: Result<[I128; 2], PromiseError>,
        zero_for_one: bool,
    ) -> U128 {
        let amounts = amounts_res.unwrap();
        if zero_for_one {
            let amount_1 = -amounts[1].0 as u128;
            U128::from(amount_1)
        } else {
            let amount_0 = -amounts[0].0 as u128;
            U128::from(amount_0)
        }
    }

    pub fn get_liquidity_for_amounts(
        &self,
        slot_0: Slot0,
        lower_tick: i32,
        upper_tick: i32,
        amount_0_desired: U128,
        amount_1_desired: U128,
    ) -> U128 {
        self.internal_calculate_liquidity(
            slot_0,
            lower_tick,
            upper_tick,
            amount_0_desired.0,
            amount_1_desired.0,
        )
        .into()
    }
}

/*
 * The rest of this file holds the inline tests for the code above
 * Learn more about Rust tests: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
 */
#[cfg(test)]
mod tests {}
