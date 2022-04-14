use serde::{Serialize, Deserialize};
use ring::{digest};

use crate::types::hash::{H256, Hashable, do_generate_random_hash};
use crate::types::transaction::SignedTransaction;
use crate::types::merkle::{MerkleTree};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rand::Rng;
use hex_literal::hex;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub length: u32, // length of the block from the genesis block

    pub header: Header,
    pub data: Vec<SignedTransaction>
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        self.header.hash()
    }
}

impl Block {
    pub fn get_parent(&self) -> H256 {
        self.header.parent
    }

    pub fn get_difficulty(&self) -> H256 {
        self.header.difficulty
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub parent: H256,
    pub nonce: u32, // public, as we have to find the correct nonce when mining
    pub difficulty: H256,
    pub timestamp: u128,
    pub merkle_root: H256
}

impl Hashable for Header {
    fn hash (&self) -> H256 {
        let serialized = serde_json::to_string(self).unwrap();
        let str_ref = serialized.as_str();

        let digest = digest::digest(&digest::SHA256, str_ref.as_bytes());

        digest.into()
    }
}

impl Hashable for SignedTransaction{
    fn hash(&self) -> H256{
        let serialized = serde_json::to_string(self).unwrap();
        let str_ref = serialized.as_str();

        let digest = digest::digest(&digest::SHA256, str_ref.as_bytes());

        digest.into()
    }
}

pub fn do_generate_random_block (parent: &H256) -> Block {
    let empty_data:Vec<SignedTransaction> = Vec::new();
    let mut rng = rand::thread_rng();
    let random_nonce:u32 = rng.gen();
    let now = SystemTime::now();
    let timestamp:u128 = now.duration_since(UNIX_EPOCH).expect("Clock may have gone backwards").as_millis();

    let difficulty_bytes = hex!("0000800000000000000000000000000000000000000000000000000000000000");
    let difficulty:H256 = difficulty_bytes.into();

    let random_merkle = do_generate_random_hash();

    let random_block = Block {
        header : Header {
            parent: *parent,
            nonce : random_nonce,
            difficulty: difficulty,
            timestamp: timestamp,
            merkle_root: random_merkle

        },
        length: 1,
        data:empty_data,
    };
    random_block
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_block(parent: &H256) -> Block {
    do_generate_random_block(parent)
}