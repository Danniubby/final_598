import requests

state1_1 = sorted(requests.get("http://127.0.0.1:7000/blockchain/state?block=0").json())
state1_2 = sorted(requests.get("http://127.0.0.1:7001/blockchain/state?block=0").json())
state1_3 = sorted(requests.get("http://127.0.0.1:7002/blockchain/state?block=0").json())

state2_1 = sorted(requests.get("http://127.0.0.1:7000/blockchain/state?block=10").json())
state2_2 = sorted(requests.get("http://127.0.0.1:7001/blockchain/state?block=10").json())
state2_3 = sorted(requests.get("http://127.0.0.1:7002/blockchain/state?block=10").json())

state3_1 = sorted(requests.get("http://127.0.0.1:7000/blockchain/state?block=20").json())
state3_2 = sorted(requests.get("http://127.0.0.1:7001/blockchain/state?block=20").json())
state3_3 = sorted(requests.get("http://127.0.0.1:7002/blockchain/state?block=20").json())

flag = True

def Diff(li1, li2):
    li_dif = [i for i in li1 + li2 if i not in li1 or i not in li2]
    return li_dif

if len(state1_1) != 1 or len(state1_2) != 1 or len(state1_3) != 1:
	print('Failure: ICO states contains more than 1 entry')
	print('Failure: ICO states contains more than 1 entry')
else:
	print('SUCCESS: ICO states only contain 1 entry')
    
if state1_1 == state1_2 and state1_2 == state1_3:
    pass
else:
    flag = False
    print("Failure:State are same for all three nodes in block0")
    print(f"DIF: 1&2 = {state1_1-state1_2}\n DIF 1&3 = {state1_1-state1_3}\n DIF 2&3 = {state1_2-state1_3}")

if state2_1 == state2_2 and state2_2 == state2_3:
    pass
else:
    flag = False
    print("Failure:State are same for all three nodes in block10")
    print(f"DIF: 1&2 = {Diff(state2_1, state2_2)}\n DIF 1&3 = {Diff(state2_1, state2_3)}\n DIF 2&3 = {Diff(state2_2, state2_3)}")

if state3_1 == state3_2 and state3_2 == state3_3:
    pass
else:
    flag = False
    print("Failure:State are same for all three nodes in block20")
    print(f"DIF: 1&2 = {Diff(state3_1, state3_2)}\n DIF 1&3 = {Diff(state3_1, state3_3)}\n DIF 2&3 = {Diff(state3_2, state3_3)}")

if flag:
    print("SUCCESS:State are same for all three nodes in all three blocks")

if state1_1 == state2_1 == state3_1:
	print('FAILURE: states should evolve')
else:
	print('SUCCESS: states evolve across blocks')

if len(state2_1) >= 3 and len(state3_1) >= 3:
    print('SUCCESS: states have >= 3 entries')
else:
    print('FAILURE: states at blocks 10 and 20 do not have >=3 entries')
