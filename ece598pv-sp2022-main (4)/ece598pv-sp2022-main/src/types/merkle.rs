use std::convert::{TryInto,TryFrom};
use super::hash::{Hashable, H256};
use ring::{digest};

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    left: Option<Box<MerkleTree>>,
    right: Option<Box<MerkleTree>>,
    hash: H256,
    index: Option<usize> // None for non-leaf nodes, Some for leaf nodes
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self where T: Hashable, {
        gen_helper(data)
    }

    pub fn root(&self) -> H256 {
        self.hash
    }

    /// Returns the Merkle Proof of data at index i
    pub fn proof(&self, index: usize) -> Vec<H256> {
        let mut accum: Vec<H256> = Vec::new();
        proof_helper(self, &mut accum, index);

        return accum;
    }
}

/// Concat left and right, and hash the output
fn concat_and_hash(left: &[u8; 32], right: &[u8; 32]) -> H256 {
    let mut concat = Vec::new();

    // concat
    concat.extend_from_slice(left);
    concat.extend_from_slice(right);

    // hash the concatenation
    let concat_hash_bytes = concat.as_ref();
    let node_hash_digest = digest::digest(&digest::SHA256, concat_hash_bytes);
    let node_hash_bytes_slice: &[u8] = node_hash_digest.as_ref();
    let node_hash_byte_arr: &[u8; 32] = node_hash_bytes_slice.try_into().expect("hash is of the wrong length");

    node_hash_byte_arr.into()
}

fn gen_helper<T>(data: &[T]) -> MerkleTree where T: Hashable, {
    // initialization
    let mut tree_nodes: Vec<MerkleTree> = Vec::new();
    for (idx, data_elem) in data.iter().enumerate() {
        let leaf = MerkleTree{left: None, right: None, hash: data_elem.hash(), index: idx.into()};
        tree_nodes.push(leaf);
    }

    // build tree from bottom up
    while tree_nodes.len() > 1 {
        if tree_nodes.len() % 2 != 0 {
            let node_to_dup = &tree_nodes[tree_nodes.len() - 1];

            let dup_node = MerkleTree{left: None, right: None, hash: node_to_dup.hash, index: None};
            tree_nodes.push(dup_node);
        }
        for _i in 0..tree_nodes.len()/2 {
            let left = tree_nodes.remove(0);
            let right = tree_nodes.remove(0);
            let hash = concat_and_hash(&(left.hash.into()), &(right.hash.into()));

            let new_node = MerkleTree{left: Some(Box::new(left)), right: Some(Box::new(right)), hash: hash, index: None};
            tree_nodes.push(new_node);
        }
    }

    tree_nodes.pop().unwrap()
}

fn proof_helper(curr: &MerkleTree, accum:  &mut Vec<H256>, index: usize) -> bool {
    // check if leaf node
    if curr.left.is_none() && curr.right.is_none() {
        if !curr.index.is_none() && curr.index.unwrap() == index {
            return true;
        } else {
            return false;
        }
    } else {
        let index_in_left_subtree = proof_helper(&(curr.left.as_ref().unwrap()), accum, index);
        let index_in_right_subtree = proof_helper(&(curr.right.as_ref().unwrap()), accum, index);

        if index_in_left_subtree {
            let right_node = curr.right.as_ref();
            let right_node_hash = right_node.as_ref().unwrap().hash;
            accum.push(right_node_hash);
        } else if index_in_right_subtree {
            let left_node = curr.left.as_ref();
            let left_node_hash = left_node.as_ref().unwrap().hash;
            accum.push(left_node_hash);
        }
        return index_in_left_subtree || index_in_right_subtree;
    }
}

fn order_proof(index: usize, leaf_size: usize) -> Vec<u32>{
    let mut order: Vec<u32> = Vec::new();
    let mut low = 0;
    let mut high = 1; // low to high, inclusive

    // rounding to nearest power of 2
    while high < leaf_size {
        high *= 2;
    }
    high -= 1;

    // pushing the order of each element of proof
    while low != high {
        let middle = (high + low)/2;
        if index <= middle {
            high = middle;
            order.push(0);
        } else {
            low = middle + 1;
            order.push(1);
        }
    }
    
    order
}

fn verify_helper(root: &H256, datum: &H256, proof: &[H256], order: &mut Vec<u32>) -> bool{
    let mut currhash = *datum ;
    let hashing_order = order;

    // loop through proof with order to generate hash
    for sib in proof.iter() {
        // concat the hashes
        let veryfing_agent:[u8; 32] = currhash.into();
        let sibling: [u8; 32] = sib.into();

        // get the hashing direction
        let direction = hashing_order.pop();
        if direction.unwrap() == 0 {
            currhash = concat_and_hash(&veryfing_agent, &sibling);
        } else {
            currhash = concat_and_hash(&sibling, &veryfing_agent);
        }
    }

    currhash == *root
}

/// Verify that the datum hash with a vector of proofs will produce the Merkle root. Also need the
/// index of datum and `leaf_size`, the total number of leaves.
pub fn verify(root: &H256, datum: &H256, proof: &[H256], index: usize, leaf_size: usize) -> bool {
    let mut order = order_proof(index, leaf_size);
    let result = verify_helper(root, datum, proof ,&mut order);

    result
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use crate::types::hash::H256;
    use super::*;

    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
            ]
        }};
    }

    macro_rules! gen_merkle_tree_data_odd {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010203")).into(),
            ]
        }};
    }

    macro_rules! gen_merkle_tree_data_big {
        () => {{
            vec![
                // (hex!("0101010101010101010101010101010101010101010101010101010101010200")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010201")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010203")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010204")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010205")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010206")).into(),
                // (hex!("0101010101010101010101010101010101010101010101010101010101010207")).into(),
                // (hex!("0101010101010101010101010101010101010101010101010101010101010208")).into(),
                // (hex!("0101010101010101010101010101010101010101010101010101010101010209")).into(),
                // (hex!("010101010101010101010101010101010101010101010101010101010101020a")).into(),
                // (hex!("010101010101010101010101010101010101010101010101010101010101020b")).into(),
                // (hex!("010101010101010101010101010101010101010101010101010101010101020c")).into(),
            ]
        }};
    }


    #[test]
    fn merkle_root_big() {
        let input_data: Vec<H256> = gen_merkle_tree_data_big!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("5fb68edb5d81005ea69380a50b964b67c974aeb9ce8059d1f5201e9f5aea7c8d")).into()
        );
   }

    #[test]
    fn merkle_root_odd() {
        let input_data: Vec<H256> = gen_merkle_tree_data_odd!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("da0d0343ec12341c3c1f3cbb859e1d90ff7a42f95154c39c70e9250f9647afca")).into()
        );
        // hash (0100...3) = 386...
        // hash (386.. || 386..) = 550f...
        // hash (6b... || 550f..) = da0...
    }

    #[test]
    fn merkle_root() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }

    #[test]
    fn merkle_proof() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(proof,
                   vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into()]
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }

    #[test]
    fn merkle_verifying() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(&merkle_tree.root(), &input_data[0].hash(), &proof, 0, input_data.len()));
    }

    #[test]
    fn merkle_verifying_odd() {
        let input_data: Vec<H256> = gen_merkle_tree_data_big!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(1);
        assert!(verify(&merkle_tree.root(), &input_data[1].hash(), &proof, 1, input_data.len()));
    }

    #[test]
    fn merkle_verifying_big() {
        let input_data: Vec<H256> = gen_merkle_tree_data_big!();
        let merkle_tree = MerkleTree::new(&input_data);
        for i in 0..input_data.len() {
            let proof = merkle_tree.proof(i);
            assert!(verify(&merkle_tree.root(), &input_data[i].hash(), &proof, i, input_data.len()));
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST