#!/usr/bin/env python3
"""Actual offline public suffix replay after a validated fresh-owner checkpoint.
No ID normalization; raw public observations and full canonical receipts retained.
Run inside the isolated network-none runner only.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading

from test_owned_evaluator import JsonLines, save, files


class Child:
    def __init__(self, binary, config, root, checkpoint=None):
        self.root, self.log = root, []
        rr, rw = os.pipe()
        sr, sw = os.pipe()
        self.rw, self.sr = rw, sr
        self.stderr = root.with_suffix('.log').open('wb')
        command = [str(binary), '--config', str(config), '--data-root', str(root),
                   '--control-read-fd', str(rr), '--control-write-fd', str(sw)]
        if checkpoint:
            command += ['--restore-checkpoint', checkpoint['manifest'], '--checkpoint-sha256', checkpoint['sha256']]
        self.command = command
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.stderr, bufsize=0, pass_fds=(rr, sw),
            env={'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': str(root.parent / 'home'),
                 'TMPDIR': str(root.parent / 'home'), 'RECALLD_EVAL_OFFLINE_KEY': 'offline-fixture'})
        os.close(rr); os.close(sw)
        self.actor = JsonLines(self.process.stdout.fileno(), self.process.stdin.fileno(), self.log)
        self.control = JsonLines(sr, rw, self.log)

    def initialize(self):
        reply = self.actor.call('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {},
            'clientInfo': {'name': 'actual-checkpoint-replay', 'version': '1'}}, public=True)
        assert 'error' not in reply, reply
        os.write(self.process.stdin.fileno(), b'{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n')

    def private(self, method, params):
        reply = self.control.call(method, params)
        assert 'error' not in reply, reply
        return reply['result']

    def tool(self, name, arguments):
        reply = self.actor.call('tools/call', {'name': name, 'arguments': arguments}, public=True)
        assert 'error' not in reply and reply['result'].get('isError') is not True, reply
        return json.loads(reply['result']['content'][0]['text'])

    def close(self):
        if self.rw is not None:
            os.close(self.rw); self.rw = None
        try:
            self.process.wait(timeout=10)
        finally:
            if self.process.poll() is None:
                self.process.kill(); self.process.wait(timeout=5)
            self.process.stdin.close(); self.process.stdout.close()
            os.close(self.sr); self.stderr.close()
            save(self.root.with_suffix('.transcript.json'), {'command': self.command, 'exitCode': self.process.returncode, 'frames': self.log})


def suffix(child, prefix, requests):
    provider_start = len(requests)
    child.private('advance', {'deltaMs': 86400000, 'deadlineMs': 30000})
    new = child.tool('store_memory', {'summary': 'suffix-known', 'fullText': 'suffix payload',
        'namespace': 'default', 'supersedes': prefix['id']})
    # Distinct vectors, explicit rank cap and no graph expansion: avoid legacy
    # equal-weight neighbor ordering. This is a bounded fixture, not all v1 ties.
    before = child.private('inspect', {})
    read = child.tool('recall_memories', {'query': 'suffix-known', 'recordAccess': False,
        'namespace': 'default', 'limit': 1})
    assert new['id'] in [r['id'] for r in read['memories']], read
    assert child.private('inspect', {}) == before
    repeated = child.tool('recall_memories', {'query': 'suffix-known', 'recordAccess': False, 'namespace': 'default', 'limit': 1})
    assert repeated == read
    query_calls = [v for v in requests[provider_start:] if v.get('input') in ('suffix-known', ['suffix-known'])]
    assert len(query_calls) == 2, query_calls
    feedback = child.tool('reinforce_memory', {'id': new['id'], 'namespace': 'default', 'quality': 3})
    forgotten = child.tool('forget_memory', {'id': prefix['id'], 'namespace': 'default'})
    state = child.private('inspect', {})
    ready = child.private('barrier', {'target': state['committed'], 'deadlineMs': 30000})
    assert ready['ready'] and ready['pendingWork'] == 0
    checkpoint = child.private('snapshot', {'deadlineMs': 30000})
    # Compare complete semantic tool values. Timing fields, if introduced, must
    # receive explicit reviewed handling rather than arbitrary field stripping.
    return {'new': new, 'query': read, 'feedback': feedback, 'forget': forgotten,
            'state': state, 'ready': ready, 'logicalHash': checkpoint['logicalHash'], 'providerRequests': requests[provider_start:]}


def run(binary, output, endpoint, requests):
    (output / 'home').mkdir(mode=0o700)
    config = output / 'config.toml'
    config.write_text(f'''[embedding]
provider = "openai"
model_name = "offline-known-v1"
api_key_env = "RECALLD_EVAL_OFFLINE_KEY"
base_url = "{endpoint}"
dimensions = 4
document_prefix = ""
query_prefix = ""
[decay]
disable_sweep = true
decay_rate_multiplier = 0.0
[rif]
enabled = false
[graph]
autolink_enabled = false
''')
    original = Child(binary, config, output / 'original')
    negative = []
    try:
        original.initialize()
        prefix = original.tool('store_memory', {'summary': 'prefix-known', 'fullText': 'prefix payload', 'namespace': 'default'})
        # Warm the identical query twice before capture; production owned constructor
        # disables CachedProvider. Each query must still reach the offline stub.
        warm_start = len(requests)
        for _ in range(2):
            original.tool('recall_memories', {'query': 'suffix-known', 'recordAccess': False, 'namespace': 'default', 'limit': 1})
        assert len(requests) - warm_start == 2
        checkpoint = original.private('snapshot', {'deadlineMs': 30000})
        save(output / 'prefix-checkpoint.json', checkpoint)
        immutable = files(Path(checkpoint['manifest']).parent)
        # A fresh clone must reject while any old engine still owns source locks.
        active = Child(binary, config, output / 'active-must-reject', checkpoint)
        try:
            assert active.process.wait(timeout=15) != 0
            assert not active.root.exists()
            negative.append('old-owner-active')
        finally:
            active.close()
        expected = suffix(original, prefix, requests)
        save(output / 'suffix-original.json', expected)
    finally:
        original.close()
    # Manifest corruption checks use new files beside the immutable manifest;
    # original evidence is never rewritten. Hashes are supervisor inputs.
    manifest_path = Path(checkpoint['manifest'])
    manifest = json.loads(manifest_path.read_text())
    cases = [('unknown-schema', {**manifest, 'checkpointVersion': 999}),
             ('unknown-field', {**manifest, 'invented': True}),
             ('clock-regressed', {**manifest, 'nowMillis': 0}),
             ('exhausted-allocation', {**manifest, 'allocation': {**manifest['allocation'], 'counter': 18446744073709551615}}),
             ('foreign-generation', {**manifest, 'generation': {**manifest['generation'], 'storeId': '00000000-0000-0000-0000-000000000001'}}),
             ('config-mismatch', {**manifest, 'config': {}})]
    for label, value in cases:
        path = manifest_path.parent / f'{label}.json'
        data = json.dumps(value).encode(); path.write_bytes(data)
        child = Child(binary, config, output / label, {'manifest': str(path), 'sha256': hashlib.sha256(data).hexdigest()})
        try:
            assert child.process.wait(timeout=15) != 0, label
            assert not child.root.exists(), label
            negative.append(label)
        finally:
            child.close()
    actual = []
    for n in range(3):
        child = Child(binary, config, output / f'restored-{n}', checkpoint)
        try:
            child.initialize()
            before = child.private('snapshot', {'deadlineMs': 30000})
            assert before['logicalHash'] == checkpoint['logicalHash']
            assert before['committed'] == checkpoint['committed']
            result = suffix(child, prefix, requests)
            save(output / f'suffix-restored-{n}.json', result)
            assert result == expected, f'actual suffix mismatch replay {n}'
            actual.append(result)
        finally:
            child.close()
    # Extra negative manifests were separate files; check all original evidence.
    for name, sha in immutable.items():
        assert hashlib.sha256((manifest_path.parent / name).read_bytes()).hexdigest() == sha
    save(output / 'summary.json', {'freshOwnerSuffixReplays': len(actual), 'negativeCases': negative,
        'checkpoint': checkpoint, 'logicalSuffixHash': expected['logicalHash'],
        'claim': 'bounded legacy-v1 fixture exact IDs/runtime/receipt logical hash and complete semantic observations; no native-v2/all-ties/full-A6 claim'})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.output.mkdir(parents=True)
    requests = []
    class Embeddings(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args): pass
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            requests.append(body)
            inputs = body.get('input', []); inputs = [inputs] if isinstance(inputs, str) else inputs
            vectors = {'prefix-known\n\nprefix payload': [1.,0.,0.,0.],
                       'suffix-known\n\nsuffix payload': [0.,1.,0.,0.], 'suffix-known': [0.,1.,0.,0.]}
            ok = body.get('model') == 'offline-known-v1' and inputs and all(v in vectors for v in inputs)
            self.send_response(200 if ok else 404); self.send_header('Content-Type', 'application/json'); self.end_headers()
            self.wfile.write(json.dumps({'data': [{'index': i, 'embedding': vectors[v]} for i,v in enumerate(inputs)],
                'usage': {'prompt_tokens': 0, 'total_tokens': 0}} if ok else {'error': 'offline replay miss'}).encode())
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Embeddings)
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    try:
        run(args.binary.resolve(), args.output, f'http://127.0.0.1:{server.server_port}/v1', requests)
    finally:
        save(args.output / 'provider-requests.json', requests)
        server.shutdown(); server.server_close(); thread.join()


if __name__ == '__main__': main()
