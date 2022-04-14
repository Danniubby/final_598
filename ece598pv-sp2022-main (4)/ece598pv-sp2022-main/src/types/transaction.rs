use serde::{Serialize,Deserialize};
use ring::signature::{self, Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm, EdDSAParameters, UnparsedPublicKey};
use rand::Rng;

use crate::network::message::Message;
use crate::types::key_pair;
use crate::types::address::Address;
use crate::types::hash::Hashable;
use crate::network::server::Handle as NetworkServerHandle;
use crate::types::hash::H256;
use crate::blockchain::Blockchain;

use std::sync::{Arc, Mutex};

use log::debug;

use std::collections::HashMap;
use std::time;
use std::thread;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    pub sender: Address,
    pub receiver: Address,
    pub account_nonce: u32,
    pub value: u32
}

// HashMap<account address, (account nonce, balance)>
pub type State = HashMap<Address, (u32, u32)>;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    let serialized = serde_json::to_string(t).unwrap();
    let str_ref = serialized.as_str();

    key.sign(str_ref.as_bytes()) // sign here and return
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    let pub_key_verifier = UnparsedPublicKey::new(&signature::ED25519, public_key);

    let serialized = serde_json::to_string(t).unwrap();
    let serialized_tx = serialized.as_str();

    match pub_key_verifier.verify(serialized_tx.as_bytes(), signature) {
        Ok(_res) => true,
        Err(_error) => false
    }
}

pub fn delete_tx_from_mempool(mempool: HashMap<H256, SignedTransaction>, to_delete: &Vec<SignedTransaction>) -> HashMap<H256, SignedTransaction> {
    let to_delete_hashes = to_delete.iter().map(|tx| tx.hash()).collect::<Vec<_>>();
    let mut new_mempool = mempool.clone();
    new_mempool.retain(|hash, _| !to_delete_hashes.contains(hash));

    return new_mempool
}

pub fn do_generate_random_transaction(parent_state: &State) -> Transaction {
    let mut rng = rand::thread_rng();

    let rand_tx: u32 = rng.gen_range(0..10);
    let rand_rx: u32 = rng.gen_range(0..10);
    let sender = Address::from_public_key_bytes(&rand_tx.to_be_bytes());
    let sender_acc_nonce = parent_state.get(&sender);
        let sender_acc_nonce = match sender_acc_nonce {
            None => 0,
            Some((acc_nonce, _)) => *acc_nonce
        };
    Transaction {
        sender: Address::from_public_key_bytes(&rand_tx.to_be_bytes()),
        receiver: Address::from_public_key_bytes(&rand_rx.to_be_bytes()),
        account_nonce: sender_acc_nonce+1,
        value: rng.gen_range(0..100)
    }
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_transaction(parent_state: &State) -> Transaction {
    do_generate_random_transaction(parent_state)
}

pub fn generate_tx_loop(theta: u64, network: NetworkServerHandle, blockchain: Arc<Mutex<Blockchain>>) {
    loop {
        let parent_state;
        {
            let blockchain = blockchain.lock().unwrap();
            parent_state = blockchain.get_block_state(&blockchain.tip()).unwrap().clone();
        }
        let random_tx = do_generate_random_transaction(&parent_state);
        let key = key_pair::random();
        let signature = sign(&random_tx, &key);
        let signed_tx = SignedTransaction {
            transaction: random_tx,
            signature: signature.as_ref().to_vec(),
            public_key: key.public_key().as_ref().to_vec(),
        };

        network.broadcast(Message::Transactions(vec![signed_tx]));

        let interval = time::Duration::from_millis(theta as u64);
        thread::sleep(interval);
    }
}

/// Execute the transactions, also performing necessary checks
pub fn execute_tx(parent_state: &State, tx_list: &Vec<SignedTransaction>) -> (State, Vec<SignedTransaction>) {
    let mut new_state = parent_state.clone();
    let mut valid_tx = vec![];

    for tx in tx_list {
        // debug!("SENDER: :{:?}, RECEIVER:{:?}",tx.transaction.sender,tx.transaction.receiver);
        let receiver = tx.transaction.receiver;
        let receiver_balance = parent_state.get(&receiver);
        let receiver_balance = match receiver_balance {
            None => 0,
            Some((_, balance)) => *balance
        };
        let receiver_acc_nonce = parent_state.get(&receiver);
        let receiver_acc_nonce = match receiver_acc_nonce {
            None => 0,
            Some((acc_nonce, _)) => *acc_nonce
        };

        let sender = tx.transaction.sender;
        let sender_balance = parent_state.get(&sender);
        let sender_balance = match sender_balance {
            None => {
                continue;
            },
            Some((_, balance)) => *balance
        };

        if sender_balance < tx.transaction.value {
            continue;
        }

        let sender_acc_nonce = parent_state.get(&sender);
        let sender_acc_nonce = match sender_acc_nonce {
            None => {
                continue;
            },
            Some((acc_nonce, _)) => *acc_nonce
        };

        if tx.transaction.account_nonce != sender_acc_nonce+1{
            continue
        }
        valid_tx.push(tx.clone());
        // debug!("inserting entry receiver: {:?}, sender: {:?}, amount:{:?}",receiver,sender,tx.transaction.value);
        // debug!("Balance after send :{:?}",sender_balance-tx.transaction.value);
        new_state.insert(sender, (tx.transaction.account_nonce, sender_balance-tx.transaction.value));
        if sender == receiver{
            new_state.insert(sender, (tx.transaction.account_nonce, sender_balance));
            // debug!("SELF SENDING!!!");
            // debug!("Balance before receive: {:?}",receiver_balance);
        }
        else{
            new_state.insert(receiver, (receiver_acc_nonce, receiver_balance+tx.transaction.value));
            // debug!("NORMAL SENDING Receiver: {:?}",receiver);
        }
    }

    (new_state, valid_tx)
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::key_pair;
    use ring::signature::KeyPair;
    
    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
    }
    #[test]
    fn sign_verify_two() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        let key_2 = key_pair::random();
        let t_2 = generate_random_transaction();
        assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
        assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST