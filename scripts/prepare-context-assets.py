#!/usr/bin/env python3
"""Install only the fixed verified wheel contents; no network, resolver, hooks or .data wheels."""
import argparse
import json
from pathlib import Path
import shutil
import zipfile
import context_tokenizer as tokenizer


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('prefetch','out'):p.add_argument('--'+name,type=Path,required=True)
    args=p.parse_args();lock=tokenizer.lock();payloads={}
    # Verify all input bytes and member hashes before creating the bundle.
    for name,sha in lock['wheels'].items():
        path=args.prefetch/'wheels'/name
        if path.is_symlink() or tokenizer.digest(path.read_bytes())!=sha:raise ValueError('wheel hash mismatch')
        with zipfile.ZipFile(path) as wheel:
            for item in wheel.infolist():
                if item.is_dir():continue
                name=item.filename;relative=Path(name)
                if relative.is_absolute() or '..' in relative.parts or name in payloads or name not in lock['installedFilesSha256']:raise ValueError('unsupported wheel member')
                value=wheel.read(item)
                if tokenizer.digest(value)!=lock['installedFilesSha256'][name]:raise ValueError('wheel member hash mismatch')
                payloads[name]=value
    if set(payloads)!=set(lock['installedFilesSha256']):raise ValueError('incomplete installed file inventory')
    rank=args.prefetch/'cl100k_base.tiktoken';config=Path(__file__).with_name('context-encoding.json')
    if tokenizer.digest(rank.read_bytes())!=lock['rankSha256'] or tokenizer.digest(config.read_bytes())!=lock['configSha256']:raise ValueError('asset hash mismatch')
    args.out.mkdir();site=args.out/'site';site.mkdir()
    for name,value in payloads.items():
        target=site/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(value)
    shutil.copyfile(rank,args.out/rank.name);shutil.copyfile(config,args.out/config.name)
    tokenizer.verify_bundle(args.out)
    (args.out/'installation.json').write_bytes(tokenizer.encoded(dict(wheels=lock['wheels'],installedFilesSha256=lock['installedFilesSha256'],tokenizer=tokenizer.identity(),method='fixed-wheel-member-copy-no-hooks-no-resolver')))


if __name__=='__main__':main()
