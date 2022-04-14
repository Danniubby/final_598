use std::cmp;
use std::collections::HashMap;

use crate::types::block::{Block, do_generate_random_block};
use crate::types::hash::{H256, Hashable, do_generate_random_hash};
use crate::types::transaction::State;
use crate::types::address::Address;

pub struct Blockchain {
    blocks: HashMap <H256, Block>,
    pub block_states: HashMap<H256, State>,
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let genesis_parent = do_generate_random_hash();
        let mut genesis_block = do_generate_random_block(&genesis_parent);
        genesis_block.length = 1;
        let genesis_hash = genesis_block.hash();

        // generate genesis block 
        let mut blocks_map = HashMap::new();
        blocks_map.insert(genesis_hash, genesis_block);

        // generate the initial ICO state
        let mut initial_block_state = HashMap::new();
        let mut ico_state = HashMap::new();
        let ico_acc: u32 = 0;
        let ico_addr = Address::from_public_key_bytes(&ico_acc.to_be_bytes());
        ico_state.insert(ico_addr, (0, 100)); // initial ico account starts with 100 coins
        initial_block_state.insert(genesis_hash, ico_state);

        Self {
            blocks: blocks_map,
            block_states: initial_block_state,
        }
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        // println!("inserting: {:?}", block.hash());
        let mut cloned_block = block.clone();

        let cloned_block_hash = cloned_block.hash();
        if cloned_block.length > 1 { // inserting a non-genesis block
            let parent_hash = cloned_block.get_parent();
            cloned_block.length = self.blocks[&parent_hash].length  + 1;
        }

        self.blocks.insert(cloned_block_hash, cloned_block);
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        let mut max_length = 0;
        let mut max_hash: H256 = [0; 32].into(); // temp value to start the search

        for (hash, block) in self.blocks.iter() {
            if block.length >= max_length {
                max_length = self.blocks[hash].length;
                max_hash = *hash;
            }
        }

        max_hash
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut curr = self.tip();
        let mut longest_chain = Vec::new();

        while self.blocks.contains_key(&curr) {
            longest_chain.push(curr);
            curr = self.blocks[&curr].get_parent();
        }

        longest_chain
    }

    // Returns a cloned block given the hash
    pub fn get_block (&self, block_hash: &H256) -> Option<&Block> {
        self.blocks.get(block_hash)
    }

    pub fn get_block_state(&self, block_hash: &H256) -> Option<&State> {
        self.block_states.get(block_hash)
    }
    
    pub fn get_parent_block(&self, block: &Block) -> Option<Block> {
        let parent_block = self.get_block(&block.get_parent());

        if parent_block.is_none() {
            return None;
        } else {
            return Some(parent_block.unwrap().clone());
        }
    }

}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;
    use ntest::timeout;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());
    }
    #[test]
    fn insert_fifty() {
        let mut blockchain = Blockchain::new();
        let mut index = 0;
        let mut genesis_hash = blockchain.tip();
        let mut block = generate_random_block(&genesis_hash);
        while index < 49{
            genesis_hash = blockchain.tip();
            block = generate_random_block(&genesis_hash);
            blockchain.insert(&block);
            index += 1;
        }
        
        assert_eq!(blockchain.tip(), block.hash());
        assert_eq!(50,blockchain.blocks[&blockchain.tip()].length);
        let longest_chain = blockchain.all_blocks_in_longest_chain();
        println!("LONGEST_CHAIN: {:?}",longest_chain);
        assert_eq!(longest_chain.len(), 50);
    }
    #[test]
    fn insert_branching() {
        let mut blockchain = Blockchain::new();
        let mut index = 0;
        let mut genesis_hash = blockchain.tip();
        let mut curtip = blockchain.tip();
        println!("GENESIS: {:?}",genesis_hash);
        let mut block = generate_random_block(&genesis_hash);
        while index < 2{
            curtip = blockchain.tip();
            block = generate_random_block(&curtip);

            blockchain.insert(&block);
            
            println!("BLOCK: {:?}",block.hash());

            index += 1;
        }
        let shorter_block = generate_random_block(&genesis_hash);
        println!("SHORTERBLOCK: {:?}",shorter_block.hash());
        blockchain.insert(&shorter_block);

        let longest_chain = blockchain.all_blocks_in_longest_chain();
        println!("LONGEST_CHAIN: {:?}",longest_chain);
        println!("TTTIIIPPP:{:?}", blockchain.tip());
        println!("ACTUALLENGTH: {:?}", blockchain.blocks.len());

        assert_eq!(3,blockchain.blocks[&blockchain.tip()].length);

    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST