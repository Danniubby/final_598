cargo build
../target/debug/bitcoin --p2p 127.0.0.1:6000 --api 127.0.0.1:7000 -vvv & ../target/debug/bitcoin --p2p 127.0.0.1:6001 --api 127.0.0.1:7001 -c 127.0.0.1:6000 -vvv &
sleep 2
echo starting miner
curl 127.0.0.1:7000/miner/start?lambda=0
curl 127.0.0.1:7001/miner/start?lambda=0
