from pathlib import Path
import hashlib,json,subprocess,sys,re
r=Path(sys.argv[1]);v=Path(sys.argv[2]);name=sys.argv[3]
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
source_file=v/(name+'-source.txt');source=source_file.read_text().strip() if source_file.exists() else subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip()
s=(v/(name+'.log')).read_text();m={'source':source,'tree':subprocess.check_output(['git','rev-parse',source+'^{tree}'],cwd=r,text=True).strip(),'exit':int((v/(name+'-exit.txt')).read_text()),'log_sha256':sha(v/(name+'.log')),'binary':[{'path':n,'sha256':sha(v/'target'/n)} for n in set(re.findall(r'\(/target/([^\)]+)\)',s)) if (v/'target'/n).is_file()],'results':[x for x in s.splitlines() if 'test result:' in x or 'generated' in x and 'warnings' in x]};(v/(name+'-manifest.json')).write_text(json.dumps(m,indent=2)+'\n');print(json.dumps(m))
