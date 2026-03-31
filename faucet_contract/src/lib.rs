use near_sdk::borsh::{self, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::{
    env, ext_contract, near, require, AccountId, BorshStorageKey, Gas, NearToken,
    PanicOnDefault, Promise, PromiseResult,
};

const ONE_DAY_NS: u64 = 86_400_000_000_000;
const FT_TRANSFER_GAS: Gas = Gas::from_tgas(30);
const CALLBACK_GAS: Gas = Gas::from_tgas(10);

#[ext_contract(ext_ft)]
trait FtContract {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[ext_contract(ext_self)]
trait FaucetCallbacks {
    fn on_claim_complete(&mut self, caller: AccountId, claim_time: u64) -> bool;
}

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Claims,
}

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct FaucetContract {
    token_contract_id: AccountId,
    claim_amount: u128,
    owner_id: AccountId,
    last_claim_at: LookupMap<AccountId, u64>,
}

#[near]
impl FaucetContract {
    #[init]
    pub fn new(token_contract_id: AccountId, claim_amount: U128, owner_id: AccountId) -> Self {
        require!(!env::state_exists(), "Already initialized");

        Self {
            token_contract_id,
            claim_amount: claim_amount.0,
            owner_id,
            last_claim_at: LookupMap::new(StorageKey::Claims),
        }
    }

    #[payable]
    pub fn claim(&mut self) -> Promise {
        require!(
            env::attached_deposit() == NearToken::from_yoctonear(1),
            "Attach exactly 1 yoctoNEAR"
        );

        let caller = env::predecessor_account_id();
        let now = env::block_timestamp();

        if let Some(last_claim_time) = self.last_claim_at.get(&caller) {
            require!(
                now >= *last_claim_time + ONE_DAY_NS,
                "You can claim only once per day"
            );
        }

        ext_ft::ext(self.token_contract_id.clone())
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(FT_TRANSFER_GAS)
            .ft_transfer(
                caller.clone(),
                U128(self.claim_amount),
                Some("faucet claim".to_string()),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_claim_complete(caller, now),
            )
    }

    #[private]
    pub fn on_claim_complete(&mut self, caller: AccountId, claim_time: u64) -> bool {
        require!(
            env::promise_results_count() == 1,
            "Expected exactly one promise result"
        );

        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                self.last_claim_at.insert(caller, claim_time);
                true
            }
            _ => false,
        }
    }

    pub fn get_claim_amount(&self) -> U128 {
        U128(self.claim_amount)
    }

    pub fn get_token_contract_id(&self) -> AccountId {
        self.token_contract_id.clone()
    }

    pub fn get_last_claim_at(&self, account_id: AccountId) -> Option<u64> {
        self.last_claim_at.get(&account_id).copied()
    }
}