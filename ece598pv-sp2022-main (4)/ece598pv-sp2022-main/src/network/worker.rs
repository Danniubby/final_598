use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;

use crate::types::hash::H256;
use crate::blockchain::Blockchain;
use crate::types::hash::Hashable;
use crate::types::block::Block;
use crate::types::address::Address;
use crate::types::transaction::{SignedTransaction, delete_tx_from_mempool, verify, execute_tx, State};

use log::{debug, warn, error};

use std::thread;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(any(test,test_utilities))]
use super::peer::TestReceiver as PeerTestReceiver;
#[cfg(any(test,test_utilities))]
use super::server::TestReceiver as ServerTestReceiver;
#[derive(Clone)]
pub struct Worker {
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    orphan_buffer: Arc<Mutex<Vec<Block>>>,
    mempool: Arc<Mutex<HashMap<H256, SignedTransaction>>>,
}

impl Worker {
    pub fn new(
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        mempool: &Arc<Mutex<HashMap<H256, SignedTransaction>>>,
    ) -> Self {
        Self {
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
            blockchain: Arc::clone(blockchain),
            orphan_buffer: Arc::new(Mutex::new(vec![])),
            mempool: Arc::clone(mempool),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    // returns hashes of blocks not seen before
    fn handle_new_block(&self, block: &Block, blockchain: &mut Blockchain) {
        debug!("Received block hash {:?} with parent hash {:?}",block.hash(), block.get_parent());

        // remove tx in block from mempool
        {
            let mut mempool = self.mempool.lock().unwrap();
            
            *mempool = delete_tx_from_mempool(mempool.clone(), &block.data);
        }

        let parent_block_option = blockchain.get_parent_block(&block);
        match parent_block_option {
            None => {
                if block.length == 1 { // new block is a genesis block, insert it
                    blockchain.insert(&block.clone());

                    // generate the initial ICO state
                    let mut ico_state = HashMap::new();
                    let ico_acc: u32 = 0;
                    let ico_addr = Address::from_public_key_bytes(&ico_acc.to_be_bytes());
                    ico_state.insert(ico_addr, (0, 100)); // initial ico account starts with 100 coins
                    blockchain.block_states.insert(block.hash(), ico_state);
                } else { // no parent block found
                    // add to orphan buffer
                    let mut orphan_buffer = self.orphan_buffer.lock().unwrap();
                    orphan_buffer.push(block.clone());

                    // broadcast the GetBlocks to get missing parent block
                    self.server.broadcast(Message::GetBlocks(vec![block.get_parent()]));
                }
            }
            Some (parent_block) => {
                let mut orphan_buffer = self.orphan_buffer.lock().unwrap();

                if check_block_validity(&block, &parent_block) {
                    let new_state = blockchain.get_block_state(&block.get_parent()).unwrap();
                    let (new_state, _) = execute_tx(&new_state, &block.data);
                    blockchain.block_states.insert(block.hash(), new_state);

                    blockchain.insert(&block.clone());
                }

                // check if any orphan blocks can be added to the blockchain
                // this has to be done until no matches are left
                let mut found = false;
                let mut new_block_inserted = block;
                let mut idx_to_delete = vec![];
                loop { // search if new_block_inserted_hash has a child in the orphan buffer
                    let mut idx = 0;

                    for block in orphan_buffer.iter() {
                        if block.get_parent() == new_block_inserted.hash() { // found a child for new_block_inserted_hash
                            if check_block_validity(block, new_block_inserted) {
                                let new_state = blockchain.get_block_state(&block.get_parent()).unwrap();
                                let (new_state, _) = execute_tx(&new_state, &block.data);
                                blockchain.block_states.insert(block.hash(), new_state);

                                blockchain.insert(&block.clone());
                            }
                            new_block_inserted = block;
                            idx_to_delete.push(idx);
                            found = true;

                            break;
                        }
                        idx += 1;
                    }

                    if !found {
                        break;
                    }

                    found = false;
                }

                // cleanup orphan buffers 
                for i in idx_to_delete.iter() {
                    orphan_buffer.remove(*i);
                }
            }
        }
    }

    fn worker_loop(self) {
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }

                // Block messages
                Message::NewBlockHashes(hashes) => {
                    let blockchain = self.blockchain.lock().unwrap();
                    let mut unseen_hashes = vec![];
                    for hash in hashes.iter() {
                        match blockchain.get_block(hash) {
                            None => {
                                unseen_hashes.push(*hash);
                            }
                            Some (_) => {
                                continue;
                            }
                        }
                    }

                    if unseen_hashes.len() > 0 {
                        peer.write(Message::GetBlocks(unseen_hashes));
                    }
                }
                Message::GetBlocks(hashes) => { 
                    let blockchain = self.blockchain.lock().unwrap();
                    let mut blocks_in_chain = vec![];
                    for hash in hashes.iter() {
                        match blockchain.get_block(hash) {
                            Some (block) => {
                                blocks_in_chain.push(block.clone());
                            }
                            None =>  {
                                continue;
                            }
                        }
                    }

                    if blocks_in_chain.len() > 0 {
                        peer.write(Message::Blocks(blocks_in_chain));
                    }
                }
                Message::Blocks(blocks) => {
                    let mut blockchain = self.blockchain.lock().unwrap();
                    let mut new_hashes = vec![];
                    for block in blocks.iter() {
                        match blockchain.get_block(&block.hash()) {
                            None => {
                                self.handle_new_block(&block, &mut blockchain);
                                new_hashes.push(block.clone().hash());
                            }
                            Some (_) => {
                                continue;
                            }
                        }
                    }
                    if new_hashes.len() > 0 {
                        self.server.broadcast(Message::NewBlockHashes(new_hashes));
                    }
                }

                // Transaction messages
                Message::NewTransactionHashes(transaction_hashes) => {
                    let mempool = self.mempool.lock().unwrap();
                    let mut unseen_hashes = vec![];
                    for hash in transaction_hashes.iter() {
                        match mempool.get(hash) {
                            None => {
                                unseen_hashes.push(*hash);
                            }
                            Some (_) => {
                                continue;
                            }
                        }
                    }

                    if unseen_hashes.len() > 0 {
                        peer.write(Message::GetTransactions(unseen_hashes));
                    }
                }
                Message::GetTransactions(transaction_hashes) => { 
                    let mempool = self.mempool.lock().unwrap();
                    let mut tx_in_mempool = vec![];
                    for tx_hash in transaction_hashes.iter() {
                        match mempool.get(tx_hash) {
                            Some (tx) => {
                                tx_in_mempool.push(tx.clone());
                            }
                            None =>  {
                                continue;
                            }
                        }
                    }

                    if tx_in_mempool.len() > 0 {
                        peer.write(Message::Transactions(tx_in_mempool));
                    }
                }
                Message::Transactions(transactions) => {
                    let mut mempool = self.mempool.lock().unwrap();
                    let mut new_hashes = vec![];
                    for tx in transactions.iter() {
                        match mempool.get(&tx.hash()) {
                            None => {
                                if check_tx_validity(tx) {
                                    mempool.insert(tx.hash(), tx.clone());
                                    new_hashes.push(tx.hash());
                                }
                            }
                            Some (_) => {
                                continue;
                            }
                        }
                    }
                    if new_hashes.len() > 0 {
                        self.server.broadcast(Message::NewTransactionHashes(new_hashes));
                    }
                }
                _ => unimplemented!(),
            }
        }
    }
}

fn check_tx_validity(tx: &SignedTransaction) -> bool {
    // signature check
    if !verify(&tx.transaction,  &tx.public_key, &tx.signature) {
        return false;
    }

    true
}

fn check_block_validity(block: &Block, parent: &Block) -> bool {
    if block.hash() > block.get_difficulty() ||  block.get_difficulty() != parent.get_difficulty() {
        return false;
    }

    true
}

#[cfg(any(test,test_utilities))]
struct TestMsgSender {
    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>
}
#[cfg(any(test,test_utilities))]
impl TestMsgSender {
    fn new() -> (TestMsgSender, smol::channel::Receiver<(Vec<u8>, peer::Handle)>) {
        let (s,r) = smol::channel::unbounded();
        (TestMsgSender {s}, r)
    }

    fn send(&self, msg: Message) -> PeerTestReceiver {
        let bytes = bincode::serialize(&msg).unwrap();
        let (handle, r) = peer::Handle::test_handle();
        smol::block_on(self.s.send((bytes, handle))).unwrap();
        r
    }
}
// #[cfg(any(test,test_utilities))]
// /// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
// fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
//     let (server, server_receiver) = ServerHandle::new_for_test();
//     let (test_msg_sender, msg_chan) = TestMsgSender::new();

//     let blockchain = Arc::new(Mutex::new(Blockchain::new()));
//     let worker = Worker::new(1, msg_chan, &server, &blockchain);
//     worker.start(); 

//     let vec_hashes;
//     {
//         let blockchain = blockchain.lock().unwrap();
//         vec_hashes = blockchain.all_blocks_in_longest_chain();
//     }

//     (test_msg_sender, server_receiver, vec_hashes)
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    use super::super::message::Message;
    use super::generate_test_worker_and_start;

    #[test]
    #[timeout(60000)]
    fn reply_new_block_hashes() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
        let reply = peer_receiver.recv();
        if let Message::GetBlocks(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_get_blocks() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let h = v.last().unwrap().clone();
        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
        let reply = peer_receiver.recv();
        if let Message::Blocks(v) = reply {
            assert_eq!(1, v.len());
            assert_eq!(h, v[0].hash())
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_blocks() {
        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
        let reply = server_receiver.recv().unwrap();
        if let Message::NewBlockHashes(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_new_block_hashes_multiple() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let first_random_block = generate_random_block(v.last().unwrap());
        let mut index = 0;
        let mut temp_tail = first_random_block;
        let mut new_hashes = vec![];
        new_hashes.push(temp_tail.hash());
        // test multiple new blocks append to the tail of blockchain
        while index < 5{
            let next_random_block = generate_random_block(&temp_tail.hash());
            temp_tail = next_random_block;
            new_hashes.push(temp_tail.hash());
            index += 1;
        }
        
        let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(new_hashes.clone()));
        let reply = peer_receiver.recv();
        if let Message::GetBlocks(v) = reply {
            assert_eq!(v, new_hashes);
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_get_blocks_multiple() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();

        let first_random_block = generate_random_block(v.last().unwrap());
        println!("first random block's parent: {:?}", first_random_block.get_parent());
        let mut new_blocks = vec![];
        let mut new_blocks_hashes = vec![];
        new_blocks.push(first_random_block.clone());
        new_blocks_hashes.push(first_random_block.hash());

        let mut append_index = 0;
        let mut temp_tail = first_random_block;   
        while append_index < 5 {
            let next_random_block = generate_random_block(&temp_tail.hash());
            temp_tail = next_random_block;
            new_blocks_hashes.push(temp_tail.hash());
            new_blocks.push(temp_tail.clone());
            append_index += 1;
        }

        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(new_blocks.clone()));

        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(new_blocks_hashes.clone()));
        let reply = peer_receiver.recv();
        if let Message::Blocks(recv_blocks) = reply {
            assert_eq!(new_blocks.len(), recv_blocks.len());
            let mut i = recv_blocks.len();
            while i > 0 {
                assert_eq!(new_blocks[i-1].hash(), recv_blocks[i-1].hash());
                i -= 1;
            }
            
        } else {
            panic!();
        }
    }

    #[test]
    #[timeout(60000)]
    fn reply_blocks_multiple() {
        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
        let first_random_block = generate_random_block(v.last().unwrap());
        let mut index = 0;
        let mut temp_tail = first_random_block;
        let mut new_blocks = vec![];
        let mut new_hashes = vec![];
        new_blocks.push(temp_tail.clone());
        new_hashes.push(temp_tail.hash());
        // test multiple new blocks append to the tail of blockchain
        while index < 5{
            let next_random_block = generate_random_block(&temp_tail.hash());
            temp_tail = next_random_block;
            new_blocks.push(temp_tail.clone());
            new_hashes.push(temp_tail.hash());
            index += 1;
        }

        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(new_blocks.clone()));
        let reply = server_receiver.recv().unwrap();
        if let Message::NewBlockHashes(v) = reply {
            assert_eq!(v, new_hashes);
        } else {
            panic!();
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST