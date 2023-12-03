use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::collections::HashMap;
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>; //Memory Type
const MAX_VALUE_SIZE: u32 = 5000; //Max Value Size

//////////////////////////////////////////////////////////////////////////
////////////////////////////// /* ENUM */ ////////////////////////////////
//////////////////////////////////////////////////////////////////////////

// Choice Enum
#[derive(Debug, CandidType, Deserialize)]
enum Choice {
    Approve,
    Reject,
}

// VoteError Enum
#[derive(Debug, CandidType, Deserialize)]
enum VoteError {
    AlreadyVoted,
    ProposalIsNotActive,
    NoSuchProposal,
    AccessRejected,
    UpdateError,
}

//////////////////////////////////////////////////////////////////////////
////////////////////////////// /* STRUCT */ //////////////////////////////
//////////////////////////////////////////////////////////////////////////

// User Struct
#[derive(Debug, CandidType, Deserialize, Clone)]
struct User {
    id: u64,
    name: String,
    address: candid::Principal,
}

// Proposal Struct
#[derive(Debug, CandidType, Deserialize, Clone)]
struct Proposal {
    id: u64,
    description: String,
    approve: u32,
    reject: u32,
    is_active: bool,
    voted: Vec<candid::Principal>,
    owner: candid::Principal,
}

// Create Proposal Struct
#[derive(Debug, CandidType, Deserialize)]
struct CreateProposal {
    description: String,
    is_active: bool,
}

//////////////////////////////////////////////////////////////////////////
///////////////////////// /* IMPLEMENTATION */ ///////////////////////////
//////////////////////////////////////////////////////////////////////////

impl Storable for Proposal {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Proposal {
    const MAX_SIZE: u32 = MAX_VALUE_SIZE;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    static PROPOSAL_ID_COUNTER: RefCell<u64> = RefCell::new(1);
    static PROPOSAL_MAP: RefCell<StableBTreeMap<u64, Proposal, Memory>> = RefCell::new(StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))));
    static USER_MAP: RefCell<HashMap<u64, User>> = RefCell::new(HashMap::new());
    static USER_ID_COUNTER: RefCell<u64> = RefCell::new(1);
}

//////////////////////////////////////////////////////////////////////////
////////////////////////// /* USER FUNCTIONS */ //////////////////////////
//////////////////////////////////////////////////////////////////////////

// Create a User Function
#[ic_cdk::update]
fn create_user(name: String, address: candid::Principal) -> u64 {
    let user_id = USER_ID_COUNTER.with(|counter| {
        let id = *counter.borrow();
        *counter.borrow_mut() += 1;
        id
    });

    let user = User {
        id: user_id,
        name,
        address,
    };

    USER_MAP.with(|u| {
        u.borrow_mut().insert(user_id, user);
    });

    user_id
}

// Get User by ID Function
#[ic_cdk::query]
fn get_user(user_id: u64) -> Option<User> {
    USER_MAP.with(|u| u.borrow().get(&user_id).cloned())
}

// Get User Count Function
#[ic_cdk::query]
fn get_user_count() -> u64 {
    USER_MAP.with(|u| u.borrow().len() as u64)
}

// Edit User Function
#[ic_cdk::update]
fn edit_user(user_id: u64, name: String, address: candid::Principal) -> Result<(), String> {
    USER_MAP.with(|u| {
        if let Some(user) = u.borrow_mut().get_mut(&user_id) {
            user.name = name;
            user.address = address;
            Ok(())
        } else {
            Err("User not found".to_string())
        }
    })
}

//////////////////////////////////////////////////////////////////////////
/////////////////////// /* PROPOSAL FUNCTIONS */ /////////////////////////
//////////////////////////////////////////////////////////////////////////

// Get Proposal by ID Function
#[ic_cdk::query] //Canister'ın sorgu fonksiyonu olduğunu belirtir.
fn get_proposal(key: u64) -> Option<Proposal> {
    PROPOSAL_MAP.with(|p| p.borrow().get(&key))
}

// Get Proposal Count Function
#[ic_cdk::query] //Canister'ın sorgu fonksiyonu olduğunu belirtir.
fn get_proposal_count() -> u64 {
    PROPOSAL_MAP.with(|p| p.borrow().len())
}

//Create a Proposal Function
#[ic_cdk::update]
fn create_proposal(proposal: CreateProposal) -> Option<Proposal> {
    let proposal_id = PROPOSAL_MAP.with(|p| p.borrow().len() as u64 + 1);

    let value = Proposal {
        id: proposal_id,
        description: proposal.description,
        approve: 0u32,
        reject: 0u32,
        is_active: proposal.is_active,
        voted: vec![],
        owner: ic_cdk::caller(),
    };

    PROPOSAL_MAP.with(|p| {
        p.borrow_mut().insert(proposal_id, value.clone());
        Some(value)
    })
}

//Edit the Proposal Function
#[ic_cdk::update]
fn edit_proposal(key: u64, proposal: CreateProposal) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        let old_proposal_opt = p.borrow().get(&key);
        let old_proposal: Proposal;

        match old_proposal_opt {
            Some(value) => old_proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }

        if old_proposal.owner != ic_cdk::caller() {
            return Err(VoteError::AccessRejected);
        }

        let value = Proposal {
            id: old_proposal.id,
            description: proposal.description,
            approve: old_proposal.approve,
            reject: old_proposal.reject,
            is_active: proposal.is_active,
            voted: old_proposal.voted,
            owner: old_proposal.owner,
        };

        let res = p.borrow_mut().insert(key, value);

        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError),
        }
    })
}

//End the Proposal Function
#[ic_cdk::update]
fn end_proposal(key: u64) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        let old_proposal_opt = p.borrow().get(&key);
        let mut old_proposal: Proposal;

        match old_proposal_opt {
            Some(value) => old_proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }

        if old_proposal.owner != ic_cdk::caller() {
            return Err(VoteError::AccessRejected);
        }

        old_proposal.is_active = false;

        let res = p.borrow_mut().insert(key, old_proposal);

        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError),
        }
    })
}

// Vote Function
#[ic_cdk::update]
fn vote(key: u64, choice: Choice) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        let proposal_opt = p.borrow().get(&key);
        let mut proposal: Proposal;

        match proposal_opt {
            Some(value) => proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }

        let caller = ic_cdk::caller();

        if proposal.voted.contains(&caller) {
            return Err(VoteError::AlreadyVoted);
        } else if proposal.is_active != true {
            return Err(VoteError::ProposalIsNotActive);
        }

        match choice {
            Choice::Approve => proposal.approve += 1,
            Choice::Reject => proposal.reject += 1,
        }

        proposal.voted.push(caller);
        let res = p.borrow_mut().insert(key, proposal);

        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError),
        }
    })
}
