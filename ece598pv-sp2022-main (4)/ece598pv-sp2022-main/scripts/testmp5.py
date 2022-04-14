import requests

chain1 = requests.get("http://127.0.0.1:7000/blockchain/longest-chain-tx").json()
chain2 = requests.get("http://127.0.0.1:7001/blockchain/longest-chain-tx").json()
chain3 = requests.get("http://127.0.0.1:7002/blockchain/longest-chain-tx").json()

def get_total_tx_num(chain):
	tx_num = 0
	for tx_list in chain:
		tx_num += len(tx_list)

	return tx_num

def get_unique_tx_num(chain):
	all_unique_tx = set()
	for tx_list in chain:
		all_unique_tx.update(set(tx_list))

	return len(all_unique_tx)

def ave_tx_per_block(chain):
	return get_total_tx_num(chain)/(len(chain)-1)

print('Total transactions per node')
print(f'node1: {get_total_tx_num(chain1)} node2: {get_total_tx_num(chain2)}  node3: {get_total_tx_num(chain3)}')
print('Target: >= 500')
print('\n')

print('Ave transactions per block')
print(f'node1: {ave_tx_per_block(chain1)} node2: {ave_tx_per_block(chain2)}  node3: {ave_tx_per_block(chain3)}')
print('Target: >=10 <=500')
print('\n')

print('Unique transactions ratio')
print(f'node1: {get_unique_tx_num(chain1)/get_total_tx_num(chain1)} node2: {get_unique_tx_num(chain2)/get_total_tx_num(chain2)}  node3: {get_unique_tx_num(chain3)/get_total_tx_num(chain3)}')
print('Target: >=0.9')
print('\n')

print('First tx in second block')
print(f'node1: {chain1[1][0]} node2: {chain2[1][0]}  node3: {chain3[1][0]}')
if chain1[1][0] == chain2[1][0] == chain3[1][0]:
    print('EQUAL!')
else:
    print('NOT EQUAL!')
print('\n')

