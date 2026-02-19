#![cfg(test)]

use super::*;
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, Env,
};

#[test]
fn test_rosca_flow_with_time_locks() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Deploy Contract & Token
    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = env.register_stellar_asset_contract(admin.clone());
    let token_client = TokenClient::new(&env, &token_admin);
    let token_admin_client = TokenAdminClient::new(&env, &token_admin);

    // 2. Create Users & Mint
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    for u in [&user1, &user2, &user3] {
        token_admin_client.mint(u, &1000);
    }

    // 3. Initialize with 1 hour duration (3600 seconds)
    let members = vec![&env, user1.clone(), user2.clone(), user3.clone()];
    let duration = 3600u64;
    let amount = 100i128;

    // Updated init call
    client.init(&admin, &members, &amount, &token_admin, &duration);

    // --- TEST: ON-TIME CONTRIBUTION ---
    env.ledger().set_timestamp(100); // Set time well before deadline
    client.contribute(&user1);
    assert_eq!(token_client.balance(&user1), 900);

    // --- TEST: LATE CONTRIBUTION REJECTION ---
    // Move time past the deadline (0 + 3600 = 3600)
    env.ledger().set_timestamp(3601);

    let result = env.as_contract(&contract_id, || {
        // This should panic because 3601 > 3600
        client.try_contribute(&user2)
    });
    assert!(result.is_err(), "Should have rejected late contribution");

    // --- TEST: ADMIN CALLING CLOSE_ROUND ---
    // User 2 and 3 didn't pay. Admin closes the round.
    client.close_round();

    // Verify State Advanced
    let (round, paid, deadline) = client.get_state();
    assert_eq!(round, 1);
    assert_eq!(paid.len(), 0);
    // New deadline should be current time (3601) + duration (3600) = 7201
    assert_eq!(deadline, 7201);

    // --- TEST: NEW ROUND CONTRIBUTIONS ---
    env.ledger().set_timestamp(4000); // Within new deadline
    client.contribute(&user1);
    assert_eq!(token_client.balance(&user1), 800); // Second contribution successful
}

#[test]
#[should_panic(expected = "Cannot close: Deadline has not passed yet")]
fn test_cannot_close_early() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let members = vec![&env, Address::generate(&env)];

    client.init(&admin, &members, &100, &Address::generate(&env), &3600);

    env.ledger().set_timestamp(500); // Way before 3600
    client.close_round();
}
#[test]
fn test_on_time_contribution() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = env.register_stellar_asset_contract(admin.clone());
    let token_admin_client = TokenAdminClient::new(&env, &token_admin);
    let token_client = TokenClient::new(&env, &token_admin);

    let user1 = Address::generate(&env);
    token_admin_client.mint(&user1, &1000);

    // Ensure there are at least 2 members so the round doesn't
    // auto-complete and reset the PaidMembers list immediately!
    let user2 = Address::generate(&env);
    let members = vec![&env, user1.clone(), user2.clone()];

    client.init(&admin, &members, &100, &token_admin, &3600);

    env.ledger().set_timestamp(1000);
    client.contribute(&user1);

    // Verify token balance decreased
    assert_eq!(token_client.balance(&user1), 900);

    // Verify state
    let (_, paid, _) = client.get_state();
    assert!(paid.contains(&user1));
}

#[test]
#[should_panic(expected = "Contribution failed: Round deadline has passed")]
fn test_late_contribution_rejection() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = env.register_stellar_asset_contract(admin.clone());

    let user1 = Address::generate(&env);
    let members = vec![&env, user1.clone()];

    // Init with 3600s duration.
    client.init(&admin, &members, &100, &token_admin, &3600);

    // 2. Try to contribute AFTER deadline (at 3601s)
    env.ledger().set_timestamp(3601);
    client.contribute(&user1); // Should panic
}

#[test]
fn test_admin_close_round() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AhjoorContract);
    let client = AhjoorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = env.register_stellar_asset_contract(admin.clone());
    let members = vec![&env, Address::generate(&env)];

    client.init(&admin, &members, &100, &token_admin, &3600);

    // 3. Admin calls close_round AFTER deadline
    env.ledger().set_timestamp(3601);
    client.close_round();

    let (round, _, _) = client.get_state();
    assert_eq!(round, 1); // Round should have advanced
}
