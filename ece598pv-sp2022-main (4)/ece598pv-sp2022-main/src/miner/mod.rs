pub mod worker;

use log::{info, debug};

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};

use std::thread;
use std::time;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use rand::Rng;

use crate::types::block::{Block, Header};
use crate::types::transaction::{SignedTransaction, delete_tx_from_mempool, execute_tx};
use crate::types::hash::{H256, Hashable, do_generate_random_hash};
use crate::blockchain::Blockchain;

const MAX_TX_PER_BLOCK: u32 = 300;

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Update, // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<Block>,

    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<HashMap<H256, SignedTransaction>>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(blockchain: &Arc<Mutex<Blockchain>>, mempool: &Arc<Mutex<HashMap<H256, SignedTransaction>>>,) -> (Context, Handle, Receiver<Block>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        blockchain: Arc::clone(blockchain),
        mempool: Arc::clone(mempool),
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle, finished_block_receiver)
}

// #[cfg(any(test,test_utilities))]
// fn test_new() -> (Context, Handle, Receiver<Block>) {
//     let blockchain = Arc::new(Mutex::new(Blockchain::new()));
//     new(&blockchain)
// }

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn miner_loop(&mut self) {
        let mut rng = rand::thread_rng();

        let mut parent_block: Block;
        let mut chain_tip;
        {
            let blockchain = self.blockchain.lock().unwrap();
            chain_tip = blockchain.tip();

            parent_block = blockchain.get_block(&chain_tip).unwrap().clone();
        }
        let genesis_block = parent_block.clone();

        // main mining loop
        loop {
            // update chain tip
            {
                let blockchain = self.blockchain.lock().unwrap();
                chain_tip = blockchain.tip();
    
                parent_block = blockchain.get_block(&chain_tip).unwrap().clone();
            }

            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                let blockchain = self.blockchain.lock().unwrap();
                                chain_tip = blockchain.tip();
                    
                                parent_block = blockchain.get_block(&chain_tip).unwrap().clone();
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            let mut block = get_block_template(&parent_block);

            block.header.nonce = rng.gen();
            if block.hash() <= block.get_difficulty() {
                debug!("Mined a block with hash {:?} and parent hash {:?}!", block.hash(), parent_block.hash());

                // add tx from mempool to blocks
                let mut tx_data: Vec<SignedTransaction> = Vec::new();
                let mut mempool = self.mempool.lock().unwrap();
                let mut counter = 0;
                for (_, tx) in mempool.clone() {
                    if counter == MAX_TX_PER_BLOCK {
                        break;
                    }
                    tx_data.push(tx);
                    counter += 1;
                }
                
                // update new state
                {
                    let mut blockchain = self.blockchain.lock().unwrap();
                    let new_state = blockchain.get_block_state(&parent_block.hash()).unwrap().clone();

                    let (new_state, valid_tx) = execute_tx(&new_state, &tx_data);
                    blockchain.block_states.insert(block.hash(), new_state);

                    block.data = valid_tx;
                }

                // delete new blocks form mempool
                *mempool = delete_tx_from_mempool(mempool.clone(), &tx_data);

                if block.get_parent() ==  genesis_block.hash() { // send the genesis block
                    self.finished_block_chan.send(genesis_block.clone()).expect("Send genesis block error");
                }
                self.finished_block_chan.send(block.clone()).expect("Send finished block error");
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}

fn get_block_template (parent_block: &Block) -> Block {
    let now = SystemTime::now();
    let timestamp: u128 = now.duration_since(UNIX_EPOCH).expect("Clock may have gone backwards").as_millis();
    Block {
        header : Header {
            parent: parent_block.hash(),
            nonce : 0, // start with a 0 nonce value
            difficulty: parent_block.get_difficulty(),
            timestamp: timestamp,
            merkle_root: do_generate_random_hash(),
        },
        length: parent_block.length + 1,
        data: Vec::new(),
    }
}


// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

// #[cfg(test)]
// mod test {
//     use ntest::timeout;
//     use crate::types::hash::Hashable;
//     use std::sync::{Arc, Mutex};
//     use crate::blockchain::Blockchain;
//     use crate::miner;

//     use std::{thread, time};
//     use std::net::{IpAddr, Ipv4Addr, SocketAddr};
//     use smol::channel;
//     use crate::network;

//     #[test]
//     #[timeout(60000)]
//     fn miner_three_block() {
//         let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
//         miner_ctx.start();
//         miner_handle.start(0);
//         let mut block_prev = finished_block_chan.recv().unwrap();

//         for _ in 0..2 {
//             let block_next = finished_block_chan.recv().unwrap();
//             assert_eq!(block_prev.hash(), block_next.get_parent());
//             block_prev = block_next;
//         }
//     }
//     #[test]
//     #[timeout(60000)]
//     fn miner_ten_block() {
//         let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
//         miner_ctx.start();
//         miner_handle.start(0);
//         let mut block_prev = finished_block_chan.recv().unwrap();

//         for _ in 0..9 {
//             let block_next = finished_block_chan.recv().unwrap();
//             assert_eq!(block_prev.hash(), block_next.get_parent());
//             block_prev = block_next;
//         }
//     }

    // #[test]
    // #[timeout(60000)] 
    // fn test_worker_insert() {
    //     let blockchain = Arc::new(Mutex::new(Blockchain::new()));
    //     let (miner_ctx, miner_handle, finished_block_chan) = miner::new(&blockchain);
        
    //     let (msg_tx, _) = channel::bounded(10000);

    //     let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    //     let (server_ctx, server) = network::server::new(socket, msg_tx).unwrap();
    //     let miner_worker_ctx = miner::worker::Worker::new(&server, finished_block_chan, &blockchain);

    //     miner_ctx.start();
    //     miner_handle.start(0);
    //     miner_worker_ctx.start();

    //     // sleep for a while to insert some blocks
    //     let one_sec = time::Duration::from_millis(1000);
    //     thread::sleep(one_sec);

    //     // assert if blocks have been inserted
    //     {
    //         let chain = blockchain.lock().unwrap();
    //         let longest_chain = chain.all_blocks_in_longest_chain();
    //         assert!(longest_chain.len() > 20) // might be flaky, depending on how many blocks were inserted 
    //     }
    // }
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST