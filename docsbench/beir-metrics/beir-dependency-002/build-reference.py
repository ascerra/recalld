import os,sys,json,hashlib,subprocess,tarfile,zipfile,shutil,sysconfig,platform,time
from pathlib import Path
out=Path('/output');pref=Path('/prefetch');site=out/'site';site.mkdir();build=out/'build';build.mkdir()
def digest(b):return hashlib.sha256(b).hexdigest()
def save(name,obj):(out/name).write_text(json.dumps(obj,indent=2)+'\n')
inv=json.loads((pref/'inventory.json').read_text());checked={}
for row in inv['artifacts']:
 p=pref/row['file'];assert p.is_file() and not p.is_symlink();assert digest(p.read_bytes())==row['sha256'];checked[row['file']]=row['sha256']
save('input-pre.json',checked)
save('toolchain.json',dict(python=sys.version,executable=sys.executable,pythonSha256=digest(Path(sys.executable).read_bytes()),platform=platform.platform(),machine=platform.machine(),include=sysconfig.get_path('include'),headerExists=(Path(sysconfig.get_path('include'))/'Python.h').exists(),cxx=subprocess.check_output(['g++','--version'],text=True)))
for wheel in sorted(pref.glob('*.whl')):
 with zipfile.ZipFile(wheel) as z:
  for item in z.infolist():
   target=site/item.filename;assert target.resolve().is_relative_to(site.resolve())
   assert ((item.external_attr>>16)&0o170000)!=0o120000
   if item.is_dir():target.mkdir(parents=True,exist_ok=True);continue
   target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(z.read(item))
def extract(archive,dest):
 with tarfile.open(archive) as t:
  for member in t.getmembers():
   target=dest/member.name;assert target.resolve().is_relative_to(dest.resolve());assert member.isdir() or member.isfile()
   if member.isdir():target.mkdir(parents=True,exist_ok=True)
   else:target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(t.extractfile(member).read())
extract(pref/'pytrec_eval-0.5.tar.gz',build);extract(pref/'trec_eval-d95ca64e14a47d763ae349fb65e6d8cde4141dbd.tar.gz',build)
src=build/'pytrec_eval-0.5';(build/'trec_eval-d95ca64e14a47d763ae349fb65e6d8cde4141dbd').rename(src/'trec_eval')
env=dict(PATH='/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',HOME='/tmp',PYTHONPATH=str(site),PYTHONDONTWRITEBYTECODE='1',OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1',SETUPTOOLS_USE_DISTUTILS='local')
dev=pref/'debian/libpython3.11-dev_3.11.2-6+deb12u6_arm64.deb'
assert digest(dev.read_bytes())=='355437c28e946ab6faeb241560c8abe693f4433ef535cabd02b690b8b29313fb'
assert digest(Path(sys.executable).read_bytes())=='9a070c8a36e16c152dae21b66aeb707f4d21b506c875e9b15edf4387fca7dcf8'
prereq=out/'prerequisite';prereq.mkdir();subprocess.run(['dpkg-deb','-x',str(dev),str(prereq)],check=True)
headers=prereq/'usr/include';assert (headers/'python3.11/Python.h').is_file()
header_hashes={str(p.relative_to(headers)):digest(p.read_bytes()) for p in sorted(headers.rglob('*')) if p.is_file()}
save('header-files.json',header_hashes)
env['CPATH']=str(headers/'python3.11')+':'+str(headers)
command=[sys.executable,'setup.py','build_ext','--inplace'];save('build-command.json',dict(argv=command,cwd=str(src),environment=env))
with (out/'build.log').open('wb') as log:r=subprocess.run(command,cwd=src,env=env,stdout=log,stderr=subprocess.STDOUT)
save('build-result.json',dict(exitCode=r.returncode));assert r.returncode==0,'reference extension build failed; retained build.log'
(site/'pytrec_eval').mkdir();shutil.copyfile(src/'py/__init__.py',site/'pytrec_eval/__init__.py')
so=list(src.glob('pytrec_eval_ext*.so'));assert len(so)==1;shutil.copyfile(so[0],site/so[0].name)
linkage=subprocess.check_output(['ldd',str(site/so[0].name)],text=True);(out/'extension-ldd.txt').write_text(linkage);assert 'libpython' not in linkage and '/output/prerequisite' not in linkage
assert digest(Path(sys.executable).read_bytes())=='9a070c8a36e16c152dae21b66aeb707f4d21b506c875e9b15edf4387fca7dcf8'
shutil.copytree(pref/'beir/beir',site/'beir')
files={str(p.relative_to(site)):digest(p.read_bytes()) for p in sorted(site.rglob('*')) if p.is_file()};save('installed-files.json',files)
probe=subprocess.run([sys.executable,'/output/reference-probe.py'],cwd='/tmp',env=env,capture_output=True,text=True);(out/'probe.stdout').write_text(probe.stdout);(out/'probe.stderr').write_text(probe.stderr);save('probe-result.json',dict(exitCode=probe.returncode));assert probe.returncode==0,'reference probe failed; retained stdout/stderr'
assert files=={str(p.relative_to(site)):digest(p.read_bytes()) for p in sorted(site.rglob('*')) if p.is_file()}
assert header_hashes=={str(p.relative_to(headers)):digest(p.read_bytes()) for p in sorted(headers.rglob('*')) if p.is_file()}
assert checked=={p:digest((pref/p).read_bytes()) for p in checked};save('input-post.json',checked);save('PASS.json',dict(installedFiles=len(files),extension=so[0].name,extensionSha256=files[so[0].name],scope='external reference import/scorer contract only; no MCP episode yet'))
print((out/'PASS.json').read_text())
