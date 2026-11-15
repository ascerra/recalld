import hashlib,json,pathlib
root=pathlib.Path(__file__).resolve().parent.parent
files=[p for name in ('src','tests','examples') for p in (root/name).rglob('*') if p.is_file()]
files += [root/name for name in ('Cargo.toml','Cargo.lock','build.rs') if (root/name).is_file()]
print(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)},indent=2,sort_keys=True))
