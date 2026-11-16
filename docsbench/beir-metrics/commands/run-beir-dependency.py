from pathlib import Path
import subprocess,json,hashlib,time
out=Path('/tmp/recalld-owned-native-controls-validation/beir-dependency-001');image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
files={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*.py')};(out/'driver-pins.json').write_text(json.dumps(files,indent=2)+'\n')
cmd=['docker','run','--name','recalld-beir-reference-001','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--pids-limit','256','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount','type=bind,src=/tmp/recalld-beir-prefetch,dst=/prefetch,readonly','--mount',f'type=bind,src={out},dst=/output','--workdir','/tmp','-e','HOME=/tmp','-e','PYTHONDONTWRITEBYTECODE=1',image,'python3','/output/build-reference.py']
(out/'container-command.json').write_text(json.dumps(cmd,indent=2)+'\n');started=time.time()
with (out/'run.log').open('wb') as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
(out/'container-inspect.json').write_text(subprocess.check_output(['docker','inspect','recalld-beir-reference-001'],text=True));subprocess.run(['docker','rm','recalld-beir-reference-001'],check=True,stdout=subprocess.DEVNULL)
assert files=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*.py')}
(out/'host-result.json').write_text(json.dumps(dict(exitCode=r.returncode,elapsedSeconds=time.time()-started,driverHashesUnchanged=True,image=image,cpus=2,memoryBytes=4294967296),indent=2)+'\n');print((out/'host-result.json').read_text());print((out/'run.log').read_text()[-4000:])
