#!/usr/bin/env python3
"""Pending owned-namespace-clock gate; consume two complete actual episode outputs."""
import json
import sys
from pathlib import Path
left,right=map(Path,sys.argv[1:3])
checks=[]
for name,relative in [('prefix','evaluator/prefix-checkpoint.json')]+[(name,f'evaluator/{name}/suffix.json') for name in ('original','restored-0','restored-1','restored-2')]:
    a,b=[json.loads((root/relative).read_text()) for root in (left,right)]
    checks.append(dict(case=name,left=a['logicalHash'],right=b['logicalHash'],equal=a['logicalHash']==b['logicalHash']))
print(json.dumps(dict(gate='independent-startup-logical-hash-equality',status='passed' if all(c['equal'] for c in checks) else 'failed',checks=checks),indent=2))
sys.exit(0 if all(c['equal'] for c in checks) else 1)
