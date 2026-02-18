#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec, token};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Members,           // Vec<Address>
    ContributionAmt,   // i128
    Token,             // Address
    CurrentRound,      // u32
    PaidMembers,       // Vec<Address> - members who paid in current round
}

#[contract]
pub struct AhjoorContract;

#[contractimpl]
impl AhjoorContract {
    pub fn init(env: Env, members: Vec<Address>, contribution_amount: i128, token: Address) {
        if env.storage().instance().has(&DataKey::Members) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Members, &members);
        env.storage().instance().set(&DataKey::ContributionAmt, &contribution_amount);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::CurrentRound, &0u32);
        env.storage().instance().set(&DataKey::PaidMembers, &Vec::<Address>::new(&env));
    }

    pub fn contribute(env: Env, contributor: Address) {
        contributor.require_auth();
        
        // 1. Check if member
        let members: Vec<Address> = env.storage().instance().get(&DataKey::Members).expect("Not initialized");
        if !members.contains(&contributor) {
            panic!("Not a member");
        }

        // 2. Check if already paid for this round
        let mut paid_members: Vec<Address> = env.storage().instance().get(&DataKey::PaidMembers).expect("Not initialized");
        if paid_members.contains(&contributor) {
            panic!("Already contributed for this round");
        }

        // 3. Mark as paid
        paid_members.push_back(contributor.clone());
        env.storage().instance().set(&DataKey::PaidMembers, &paid_members);

        // 4. Transfer funds
        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        let amount: i128 = env.storage().instance().get(&DataKey::ContributionAmt).unwrap();

        client.transfer(&contributor, &env.current_contract_address(), &amount);

        // 5. Check if round is complete
        if paid_members.len() == members.len() {
            // Payout
            let current_round: u32 = env.storage().instance().get(&DataKey::CurrentRound).unwrap();
            
            // Get recipient based on round (round-robin)
            // Note: `get` returns Option<Result<Address, Error>>, need to unwrap twice carefully for valid index
            let recipient_idx = current_round % members.len(); 
            let payout_recipient = members.get(recipient_idx).unwrap(); // Should be safe if members.len() > 0

            let total_pot = amount * (members.len() as i128);
            client.transfer(&env.current_contract_address(), &payout_recipient, &total_pot);

            // Increment round
            let next_round = current_round + 1;
            env.storage().instance().set(&DataKey::CurrentRound, &next_round);
            
            // Reset paid members
            env.storage().instance().set(&DataKey::PaidMembers, &Vec::<Address>::new(&env));
        }
    }
    
    pub fn get_state(env: Env) -> (u32, Vec<Address>) {
        let current_round: u32 = env.storage().instance().get(&DataKey::CurrentRound).unwrap_or(0);
        let paid_members: Vec<Address> = env.storage().instance().get(&DataKey::PaidMembers).unwrap_or(Vec::new(&env));
        (current_round, paid_members)
    }
}

mod test;
