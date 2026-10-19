"""Exercise FilmCraft's actual stdio MCP server without a model or remote client."""
import subprocess,json,sys,threading,queue
from pathlib import Path
root=Path(__file__).resolve().parent.parent
exe=root/'target/debug/filmcraft-cli.exe'
p=subprocess.Popen([str(exe),'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8')
q=queue.Queue()
def reader():
    for line in p.stdout:q.put(json.loads(line))
threading.Thread(target=reader,daemon=True).start()
transcript=[]
def send(v):
    p.stdin.write(json.dumps(v)+'\n');p.stdin.flush()
    if 'id' not in v:return
    while True:
        r=q.get(timeout=30);transcript.append(r)
        if r.get('id')==v['id']:
            if 'error' in r:raise RuntimeError(r)
            return r
try:
    send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'Reaction Studio acceptance','version':'0.1'}}})
    send({'jsonrpc':'2.0','method':'notifications/initialized'})
    listing=send({'jsonrpc':'2.0','id':2,'method':'tools/list','params':{}})
    names=[t['name'] for t in listing['result']['tools']]
    if not {'command_run','command_list','sequence_inspect'}.issubset(names):raise RuntimeError(names)
    for i,(cmd,args) in enumerate([('file.newProject',{'name':'MCP acceptance'}),('file.newSequence',{'name':'Test','width':640,'height':360,'fps':24}),('sequence.inspect',{})],3):
        r=send({'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':'command_run','arguments':{'id':cmd,'params':args}}})
        if r['result'].get('isError'):raise RuntimeError(r)
    dest=root/'target/reaction-tests/mcp-acceptance.json';dest.parent.mkdir(parents=True,exist_ok=True);dest.write_text(json.dumps({'passed':True,'tools':names,'responses':transcript},indent=2))
    print('MCP handshake, tools/list, project creation, sequence creation and inspection passed.')
finally:
    p.terminate();p.wait(timeout=15)

