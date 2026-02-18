#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Address, Env};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;

#[test]
fn test_rosca_flow() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Deploy Contract
    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    // 2. Deploy Token
    let admin = Address::generate(&env);
    let token_admin = env.register_stellar_asset_contract(admin); 
    let token_client = TokenClient::new(&env, &token_admin);
    let token_admin_client = TokenAdminClient::new(&env, &token_admin);

    // 3. Create Users
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    // Mint tokens
    token_admin_client.mint(&user1, &1000);
    token_admin_client.mint(&user2, &1000);
    token_admin_client.mint(&user3, &1000);

    // 4. Initialize
    let members = vec![&env, user1.clone(), user2.clone(), user3.clone()];
    let contribution_amount = 100i128;

    client.init(&members, &contribution_amount, &token_admin);

    // 5. Round 0 Contributions
    client.contribute(&user1);
    assert_eq!(token_client.balance(&contract_id), 100);

    client.contribute(&user2);
    assert_eq!(token_client.balance(&contract_id), 200);

    // Trigger payout (User 3)
    client.contribute(&user3);

    // Check Payout (User 1 should receive 300)
    // User 1 start: 1000 - 100 = 900. Receive 300 -> 1200.
    assert_eq!(token_client.balance(&user1), 1200);
    assert_eq!(token_client.balance(&user2), 900);
    assert_eq!(token_client.balance(&user3), 900);
    assert_eq!(token_client.balance(&contract_id), 0);

    // Check state reset
    // No explicit getter in test usually, but we can verify by starting next round
    
    // 6. Round 1 Contributions
    // User 1 contributes
    client.contribute(&user1);
    assert_eq!(token_client.balance(&contract_id), 100);
    assert_eq!(token_client.balance(&user1), 1100);

    // User 2 contributes
    client.contribute(&user2);
    
    // User 3 contributes -> Payout to User 2
    client.contribute(&user3);

    // User 2 start: 900 - 100 = 800. Receive 300 -> 1100.
    assert_eq!(token_client.balance(&user2), 1100);
    assert_eq!(token_client.balance(&contract_id), 0);
}
