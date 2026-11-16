#!/usr/bin/env python3
"""Six bounded diagnostic cells; no retries, tuning or performance promotion."""
import argparse, collections, datetime, hashlib, json, pathlib, subprocess, time
CELLS = [('adapter100','adapter-explicit',100,0,False),('mcp100','mcp-local-provider',100,0,False),('daemon100','daemon-explicit',100,0,False),('daemon1000','daemon-explicit',1000,0,False),('history1000','daemon-explicit',100,1000,False),('edges99','daemon-explicit',100,0,True)]
def now(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def capture():
    result = {'utc':now()}
    for key, command in [('host', ['top','-l','1','-n','0']),('containers',['docker','ps','--format','{{.Image}}']),('stats',['docker','stats','--no-stream','--format','{{.CPUPerc}} {{.MemUsage}} {{.PIDs}}'])]:
        p = subprocess.run(command,capture_output=True,text=True,timeout=15)
        if key=='host': value=[line for line in p.stdout.splitlines() if line.startswith(('CPU usage:','Load Avg:','PhysMem:'))]
        elif key=='containers': value=dict(collections.Counter(p.stdout.splitlines()))
        else: value=p.stdout.splitlines() # no project/container names or IDs
        result[key]={'exit':p.returncode,'value':value}
    return result
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--root',type=pathlib.Path,required=True);p.add_argument('--image',required=True);a=p.parse_args()
    a.root.mkdir(parents=True,exist_ok=False)
    manifest={'start':now(),'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'image':a.image,'cells':[]}
    def save(): (a.root/'status.json').write_text(json.dumps(manifest,indent=2)+'\n')
    save()
    for cell,route,n,h,e in CELLS:
        root=a.root/cell;root.mkdir();name='recalld-ingestion-'+cell
        entry={'cell':cell,'path':route,'records':n,'history':h,'edges':e,'before':capture(),'start':now()};manifest['cells'].append(entry);save()
        command=['docker','run','--rm','--name',name,'--network','none','--cpus','2','--memory','4g','-e','RECALLD_DIAGNOSTIC_ISOLATED=1','-e','RECALLD_DAEMON_SOCKET=/study/daemon.sock','-v',str(a.binary.resolve())+':/probe:ro','-v',str(root.resolve())+':/study',a.image,'strace','-f','-ttt','-T','-y','-e','trace=fsync,fdatasync,msync','-o','/study/syscalls.log','/probe','--root','/study','--path',route,'--records',str(n),'--history',str(h)]
        if e: command.append('--edges')
        entry['command']=command;save()
        with (root/'stdout.log').open('w') as out,(root/'stderr.log').open('w') as err:
            try: entry['exit']=subprocess.run(command,stdout=out,stderr=err,timeout=180).returncode
            except subprocess.TimeoutExpired: entry['exit']='timeout'
            finally:
                # Restrict cleanup to this unique study container, retain output.
                cleanup=subprocess.run(['docker','rm','-f',name],capture_output=True,text=True)
                entry['cleanup_exit']=cleanup.returncode
        entry['end']=now();entry['after']=capture();save();print(cell,entry['exit'],flush=True)
        if entry['exit']!=0: break
    manifest['end']=now();manifest['all_six_complete']=len(manifest['cells'])==6 and all(c['exit']==0 for c in manifest['cells']);save()
    raise SystemExit(0 if manifest['all_six_complete'] else 1)
