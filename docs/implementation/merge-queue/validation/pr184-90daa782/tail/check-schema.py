import pathlib,json,hashlib,zipfile,sys,runpy,importlib.metadata
out=pathlib.Path('/work/output');target=out/'schema-python';target.mkdir()
for name,sha in json.loads((out/'schema-wheels.json').read_text()).items():
 p=pathlib.Path('/work/schema-wheels')/name
 assert hashlib.sha256(p.read_bytes()).hexdigest()==sha
 with zipfile.ZipFile(p) as z:
  assert all(not pathlib.PurePosixPath(n).is_absolute() and '..' not in pathlib.PurePosixPath(n).parts for n in z.namelist())
  z.extractall(target)
sys.path.insert(0,str(target))
assert importlib.metadata.version('jsonschema')=='4.25.1'
fixture_count=sum(len(v) if isinstance(v,list) else 1 for v in (json.loads(p.read_text()) for p in (out/'schema-fixtures').glob('*.json')))
assert fixture_count>=61,fixture_count
sys.argv=['tests/query_schema_conformance.py','/work/output/schema-fixtures']
runpy.run_path('/work/source/tests/query_schema_conformance.py',run_name='__main__')
