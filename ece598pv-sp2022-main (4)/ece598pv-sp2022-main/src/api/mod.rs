use serde::Serialize;
use crate::blockchain::Blockchain;
use crate::miner::Handle as MinerHandle;
use crate::types::hash::Hashable;
use crate::network::server::Handle as NetworkServerHandle;
use crate::network::message::Message;
use crate::types::transaction::generate_tx_loop;
use crate::types::block::Block;
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server as HTTPServer;
use url::Url;

pub struct Server {
    handle: HTTPServer,
    miner: MinerHandle,
    network: NetworkServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

macro_rules! respond_result {
    ( $req:expr, $success:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let payload = ApiResponse {
            success: $success,
            message: $message.to_string(),
        };
        let resp = Response::from_string(serde_json::to_string_pretty(&payload).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}
macro_rules! respond_json {
    ( $req:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let resp = Response::from_string(serde_json::to_string(&$message).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}

impl Server {
    pub fn start(
        addr: std::net::SocketAddr,
        miner: &MinerHandle,
        network: &NetworkServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
    ) {
        let handle = HTTPServer::http(&addr).unwrap();
        let server = Self {
            handle,
            miner: miner.clone(),
            network: network.clone(),
            blockchain: Arc::clone(blockchain),
        };
        thread::spawn(move || {
            let started_tx_gen = Arc::new(Mutex::new(false));

            for req in server.handle.incoming_requests() {
                let miner = server.miner.clone();
                let network = server.network.clone();
                let blockchain = Arc::clone(&server.blockchain);
                let started_tx_gen = Arc::clone(&started_tx_gen);
                thread::spawn(move || {
                    // a valid url requires a base
                    let base_url = Url::parse(&format!("http://{}/", &addr)).unwrap();
                    let url = match base_url.join(req.url()) {
                        Ok(u) => u,
                        Err(e) => {
                            respond_result!(req, false, format!("error parsing url: {}", e));
                            return;
                        }
                    };
                    match url.path() {
                        "/miner/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let lambda = match params.get("lambda") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing lambda");
                                    return;
                                }
                            };
                            let lambda = match lambda.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing lambda: {}", e)
                                    );
                                    return;
                                }
                            };
                            miner.start(lambda);
                            respond_result!(req, true, "ok");
                        }
                        "/tx-generator/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let theta = match params.get("theta") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing theta");
                                    return;
                                }
                            };
                            let theta = match theta.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing theta: {}", e)
                                    );
                                    return;
                                }
                            };

                            let mut started_tx_gen = started_tx_gen.lock().unwrap();
                            if !*started_tx_gen {
                                *started_tx_gen = true;
                                generate_tx_loop(theta, network, blockchain);

                                respond_result!(req, true, "started tx generator!");
                            } else {
                                respond_result!(req, true, "already started tx generator!");
                            }
                        }
                        "/network/ping" => {
                            network.broadcast(Message::Ping(String::from("Test ping")));
                            respond_result!(req, true, "ok");
                        }
                        "/blockchain/longest-chain" => {
                            let blockchain = blockchain.lock().unwrap();
                            let v = blockchain.all_blocks_in_longest_chain();
                            let v_string: Vec<String> = v.into_iter().map(|h|h.to_string()).collect();
                            respond_json!(req, v_string);
                        }
                        "/blockchain/longest-chain-tx" => {
                            let blockchain = blockchain.lock().unwrap();
                            let v = blockchain.all_blocks_in_longest_chain();
                            // let all_blocks:Vec<Block> = v.into_iter().map(|b_hash| blockchain.get_block(&b_hash)).collect();
                            // let all_transactions: vec<String> = all_blocks.into_iter().map(|b| b.data.into_iter(|tx| tx.hash().to_string).collect()).collect();
                            let mut all_blocks: Vec<Block> = Vec::new();
                            let mut all_tx:Vec<Vec<String>>=Vec::new();
                            for block_hash in v.iter().rev() {
                                let block = blockchain.get_block(&block_hash).unwrap();
                                all_blocks.push(block.clone());
                                let mut block_txs : Vec<String> = Vec::new();
                                for tx in block.data.clone(){
                                    block_txs.push(tx.hash().to_string());
                                }
                                all_tx.push(block_txs.clone());
                            } 
                            // let v_string: Vec<String> = v.into_iter().map(|h|h.to_string()).collect();
                            respond_json!(req, all_tx);
                            // respond_result!(req, false, "unimplemented!");
                        }
                        "/blockchain/longest-chain-tx-count" => {
                            // unimplemented!()
                            respond_result!(req, false, "unimplemented!");
                        }
                        "/blockchain/state" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let block = match params.get("block") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing block count");
                                    return;
                                }
                            };
                            let block = match block.parse::<usize>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing block count: {}", e)
                                    );
                                    return;
                                }
                            };
                            let blockchain = blockchain.lock().unwrap();
                            let longest_chain_hashes = blockchain.all_blocks_in_longest_chain();

                            if block >= longest_chain_hashes.len() {
                                respond_result!(req, false, format!("block count is too large"));
                                return;
                            }

                            let target_hash = longest_chain_hashes[longest_chain_hashes.len() - block - 1];
                            let target_state_option = blockchain.get_block_state(&target_hash);
                            let target_state = match target_state_option {
                                Some(state) => state,
                                None => {
                                    respond_result!(req, false, "invalid block count");
                                    return;
                                }
                            };

                            let mut all_addr_states: Vec<String> = Vec::new();
                            for (addr, (nonce, balance)) in target_state {
                                let addr_serialized = serde_json::to_string(addr).unwrap();
                                let state_string: String = addr_serialized + " " + &nonce.to_string() + " " + &balance.to_string();
                                all_addr_states.push(state_string);
                            }

                            respond_json!(req, all_addr_states);
                        }
                        _ => {
                            let content_type =
                                "Content-Type: application/json".parse::<Header>().unwrap();
                            let payload = ApiResponse {
                                success: false,
                                message: "endpoint not found".to_string(),
                            };
                            let status_code: u16 = 404;
                            let resp = Response::from_string(
                                serde_json::to_string_pretty(&payload).unwrap(),
                            )
                            .with_header(content_type)
                            .with_status_code(status_code);
                            req.respond(resp).unwrap();
                        }
                    }
                });
            }
        });
        info!("API server listening at {}", &addr);
    }
}
