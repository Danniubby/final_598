import subprocess
import tempfile
import struct
msg = []
clients = []
with tempfile.TemporaryFile() as tempf:
    proc = subprocess.Popen(['bash', 'get_chain.sh'], stdout=tempf)
    proc.wait()
    tempf.seek(0)
    for data in enumerate(tempf.readlines()):
        # msg.append(line.split(","))
        # (i,), data = struct.unpack("I", data[1][:4]), data[1][4:]
        # s, data = data[:i], data[i:]
        
        msg.append(data[1].decode("utf-8"))
    # print(msg[0])
    for line in msg:
        clients.append(line.split(","))
    client1_length = len(clients[0])
    client2_length = len(clients[1])
    client3_length = len(clients[2])
    min = client1_length
    if min > client2_length:
        min = client2_length
    if min > client3_length:
        min = client3_length
    max = client1_length
    if min < client2_length:
        max = client2_length
    if min < client3_length:
        max = client3_length
    prefix_dif = 0
    length_dif = max - min
    for i in range(min):
        if clients[0][i] == clients[1][i] == clients[2][i]:
            continue
        else:
            prefix_dif += 1
    prefix_dif += length_dif
    print(f" Client1 num_nodes:{client1_length}\n Client2 num_nodes:{client2_length}\n \
Client3 num_nodes:{client3_length}\n Length_diff ={length_dif}\n prefix_diff = {prefix_dif}")