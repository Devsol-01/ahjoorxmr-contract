#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, token, Address, Env, Vec};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,           // Address
    Members,         // Vec<Address>
    ContributionAmt, // i128
    Token,           // Address
    CurrentRound,    // u32
    PaidMembers,     // Vec<Address>
    RoundDuration,   // u64
    RoundDeadline,   // u64
    Defaulters,      // Vec<Address> for the most recent closed round
}

#[contract]
pub struct AhjoorContract;

#[contractimpl]
impl AhjoorContract {
    /// Initializes the contract with members, contribution rules, and timing.
    pub fn init(
        env: Env,
        admin: Address,
        members: Vec<Address>,
        contribution_amount: i128,
        token: Address,
        round_duration: u64,
    ) {
        if env.storage().instance().has(&DataKey::Members) {
            panic!("Already initialized");
        }

        let start_time = env.ledger().timestamp();
        let deadline = start_time + round_duration;

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Members, &members);
        env.storage()
            .instance()
            .set(&DataKey::ContributionAmt, &contribution_amount);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::CurrentRound, &0u32);
        env.storage()
            .instance()
            .set(&DataKey::PaidMembers, &Vec::<Address>::new(&env));

        // Time-lock parameters
        env.storage()
            .instance()
            .set(&DataKey::RoundDuration, &round_duration);
        env.storage()
            .instance()
            .set(&DataKey::RoundDeadline, &deadline);
        env.storage()
            .instance()
            .set(&DataKey::Defaulters, &Vec::<Address>::new(&env));
    }

    pub fn contribute(env: Env, contributor: Address) {
        contributor.require_auth();

        // 1. Check Deadline Enforcement
        let deadline: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoundDeadline)
            .expect("Deadline not set");
        if env.ledger().timestamp() > deadline {
            panic!("Contribution failed: Round deadline has passed");
        }

        // 2. Check if member
        let members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Members)
            .expect("Not initialized");
        if !members.contains(&contributor) {
            panic!("Not a member");
        }

        // 3. Check if already paid for this round
        let mut paid_members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PaidMembers)
            .expect("Not initialized");
        if paid_members.contains(&contributor) {
            panic!("Already contributed for this round");
        }

        // 4. Transfer funds
        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        let amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ContributionAmt)
            .unwrap();

        client.transfer(&contributor, &env.current_contract_address(), &amount);

        // 5. Mark as paid
        paid_members.push_back(contributor.clone());
        env.storage()
            .instance()
            .set(&DataKey::PaidMembers, &paid_members);

        // 6. Check if round is complete (Auto-payout if everyone paid before deadline)
        if paid_members.len() == members.len() {
            Self::complete_round_payout(&env, &members, &paid_members, amount, client);
        }
    }

    /// Admin-only function to force-close an expired round and track defaulters.
    pub fn close_round(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        admin.require_auth();

        let deadline: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoundDeadline)
            .unwrap();
        if env.ledger().timestamp() <= deadline {
            panic!("Cannot close: Deadline has not passed yet");
        }

        let members: Vec<Address> = env.storage().instance().get(&DataKey::Members).unwrap();
        let paid_members: Vec<Address> =
            env.storage().instance().get(&DataKey::PaidMembers).unwrap();

        // Identify and store defaulters
        let mut defaulters = Vec::new(&env);
        for member in members.iter() {
            if !paid_members.contains(&member) {
                defaulters.push_back(member);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::Defaulters, &defaulters);

        // Advance to next round state
        let current_round: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CurrentRound)
            .unwrap();
        let duration: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoundDuration)
            .unwrap();

        env.storage()
            .instance()
            .set(&DataKey::CurrentRound, &(current_round + 1));
        env.storage()
            .instance()
            .set(&DataKey::PaidMembers, &Vec::<Address>::new(&env));
        env.storage().instance().set(
            &DataKey::RoundDeadline,
            &(env.ledger().timestamp() + duration),
        );

        // Emit event for transparency
        env.events()
            .publish((symbol_short!("closed"), current_round), defaulters);
    }

    // --- Internal Helper ---

    fn complete_round_payout(
        env: &Env,
        members: &Vec<Address>,
        paid_members: &Vec<Address>,
        amount: i128,
        client: token::Client,
    ) {
        let current_round: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CurrentRound)
            .unwrap();

        // Payout to current recipient (round-robin)
        let recipient_idx = current_round % members.len();
        let payout_recipient = members.get(recipient_idx).unwrap();

        let total_pot = amount * (paid_members.len() as i128);
        client.transfer(
            &env.current_contract_address(),
            &payout_recipient,
            &total_pot,
        );

        // Reset for next round
        let duration: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoundDuration)
            .unwrap();
        env.storage()
            .instance()
            .set(&DataKey::CurrentRound, &(current_round + 1));
        env.storage()
            .instance()
            .set(&DataKey::PaidMembers, &Vec::<Address>::new(env));
        env.storage().instance().set(
            &DataKey::RoundDeadline,
            &(env.ledger().timestamp() + duration),
        );
    }

    pub fn get_state(env: Env) -> (u32, Vec<Address>, u64) {
        let current_round: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CurrentRound)
            .unwrap_or(0);
        let paid_members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PaidMembers)
            .unwrap_or(Vec::new(&env));
        let deadline: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoundDeadline)
            .unwrap_or(0);
        (current_round, paid_members, deadline)
    }
}

mod test;
