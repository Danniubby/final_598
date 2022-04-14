# put this file, add_test.py, and your netid.zip file in a new directory
for zipfile in *.zip; do
    netid=${zipfile%%.*}
    unzip -qq $zipfile -d $netid
	if [ -d $netid ]; then
		echo "student netid: $netid" >> log.txt
        cd $netid/ece598pv-sp2022-main
        cargo build
        ./target/debug/bitcoin --p2p 127.0.0.1:6000 --api 127.0.0.1:7000 -vvv & ./target/debug/bitcoin --p2p 127.0.0.1:6001 --api 127.0.0.1:7001 -c 127.0.0.1:6000 -vvv & ./target/debug/bitcoin --p2p 127.0.0.1:6002 --api 127.0.0.1:7002 -c 127.0.0.1:6001 -vvv &
        sleep 2
        echo "starting miner">> log.txt
        curl 127.0.0.1:7000/miner/start?lambda=0
        curl 127.0.0.1:7001/miner/start?lambda=0
        curl 127.0.0.1:7002/miner/start?lambda=0
        sleep 5m
        echo "5min reached">> log.txt
        curl 127.0.0.1:7000/blockchain/longest-chain
        echo "\n"
        curl 127.0.0.1:7001/blockchain/longest-chain 
        echo "\n"
        curl 127.0.0.1:7002/blockchain/longest-chain
        echo "\n"
		
		cd ../..
	fi
done
#grep 'student netid\|test result' log.txt > result.txt