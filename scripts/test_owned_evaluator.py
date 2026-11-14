#!/usr/bin/env python3
"""Actual inherited-pipe constructor probe. Run only in isolated offline Docker.
This is contract evidence, not a deterministic snapshot/reset or quality scorer.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import select
import subprocess
import threading
import time


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


class JsonLines:
    def __init__(self, read_fd, write_fd, log):
        self.read_fd, self.write_fd, self.log = read_fd, write_fd, log
        self.pending = b''
        self.sequence = 0

    def call(self, method, params, public=False, sent=None):
        self.sequence += 1
        request = {'id': self.sequence, 'method': method, 'params': params}
        if public:
            request['jsonrpc'] = '2.0'
        sent_at = time.monotonic()
        frame = json.dumps(request).encode() + b'\n'
        while frame:
            frame = frame[os.write(self.write_fd, frame):]
        if sent is not None:
            sent.set()
        deadline = time.monotonic() + 35
        while b'\n' not in self.pending:
            remaining = deadline - time.monotonic()
            assert remaining > 0 and select.select([self.read_fd], [], [], remaining)[0], 'response timeout'
            chunk = os.read(self.read_fd, 65536)
            assert chunk, 'unexpected child EOF'
            self.pending += chunk
        line, self.pending = self.pending.split(b'\n', 1)
        response = json.loads(line)
        self.log.append({'channel': 'actor' if public else 'evaluator', 'request': request, 'response': response, 'sentMonotonic': sent_at, 'receivedMonotonic': time.monotonic()})
        assert response['id'] == request['id'], response
        return response


def files(root):
    # Observe bytes, never label copied live files a canonical snapshot.
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'fts.db-shm'}


def episode(binary, output, endpoint, index, hold, entered, resume):
    root = output / f'replay-{index}'
    root.mkdir()
    home = root / 'home'
    home.mkdir(mode=0o700)
    config = root / 'config.toml'
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
''')
    request_read, request_write = os.pipe()
    response_read, response_write = os.pipe()
    log = []
    stderr = (root / 'child.log').open('wb')
    command = [str(binary), '--config', str(config), '--data-root', str(root / 'data'),
               '--control-read-fd', str(request_read), '--control-write-fd', str(response_write)]
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                             bufsize=0, pass_fds=(request_read, response_write),
                             env={'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': str(home),
                                  'TMPDIR': str(home), 'RECALLD_EVAL_OFFLINE_KEY': 'offline-fixture'})
    os.close(request_read)
    os.close(response_write)
    actor = JsonLines(child.stdout.fileno(), child.stdin.fileno(), log)
    control = JsonLines(response_read, request_write, log)
    try:
        def rpc(method, params):
            result = actor.call(method, params, public=True)
            assert 'error' not in result, result
            return result['result']

        def tool(name, arguments, ok=True):
            result = rpc('tools/call', {'name': name, 'arguments': arguments})
            assert (result.get('isError') is not True) == ok, result
            return json.loads(result['content'][0]['text']) if ok else result

        def inspect():
            return control.call('inspect', {})['result']['committed']

        def barrier(at):
            result = control.call('barrier', {'target': at, 'deadlineMs': 30000})
            assert 'error' not in result, result
            result = result['result']
            assert result['ready'] and result['pendingWork'] == 0, result
            assert result['storeId'] == at['storeId']
            assert all(v['ready'] and v['appliedGeneration'] == result['committedGeneration'] for v in result['derived'])
            return result

        init = rpc('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {},
                                 'clientInfo': {'name': 'owned-control-probe', 'version': '1'}})
        assert init['protocolVersion'] == '2025-06-18'
        os.write(child.stdin.fileno(), b'{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n')
        names = {v['name'] for v in rpc('tools/list', {})['tools']}
        assert not names.intersection({'inspect', 'barrier', 'snapshot', 'reset', 'advance'})
        assert 'error' in actor.call('barrier', {}, public=True)
        capabilities = control.call('capabilities', {})['result']
        assert capabilities['guarantees']['snapshot'] is False
        assert capabilities['guarantees']['controlledIds'] is False
        assert capabilities['guarantees']['lifecycleClockControlled'] is True
        assert control.call('advance', {'deltaMs': 0, 'deadlineMs': 30000})['result']['nowMillis'] == 1767225600000
        for params in [{'deltaMs': -1, 'deadlineMs': 100}, {'deltaMs': 1.5, 'deadlineMs': 100},
                       {'deltaMs': 1, 'deadlineMs': 0}, {'deltaMs': 1, 'deadlineMs': 30001},
                       {'deltaMs': 1, 'deadlineMs': 100, 'unknown': True}]:
            assert control.call('advance', params)['error']['code'] == 'INVALID_PARAMS'
        assert 'error' in control.call('advance', {'deltaMs': 9223372036854775807, 'deadlineMs': 30000})
        assert control.call('advance', {'deltaMs': 60000, 'deadlineMs': 30000})['result']['nowMillis'] == 1767225660000
        runtime_initial = control.call('inspect', {})['result']
        assert runtime_initial['runtimeState']['nowMillis'] == 1767225660000
        assert runtime_initial['runtimeState']['allocation']['version'] == 1
        initial = inspect()
        barrier(initial)
        namespace = tool('create_namespace', {'name': 'owned-clock', 'embeddingDim': 4})
        assert namespace['createdAt'] == '2026-01-01 00:01:00 UTC', namespace
        # Namespace creation on this legacy parent does not itself publish all
        # derived views. The actual store below publishes before inspection.
        # Real actor work is admitted before the fixture provider blocks. A
        # private barrier targeting the prior G must drain this accepted work
        # and its insertion/publication descendants before replying.
        hold.set()
        entered.clear()
        resume.clear()
        stored_values, barrier_values = [], []
        actor_done, barrier_done, barrier_sent = threading.Event(), threading.Event(), threading.Event()
        def store_pending():
            try:
                stored_values.append(tool('store_memory', {'summary': 'offline-known', 'fullText': 'offline payload', 'namespace': 'default'}))
            finally:
                actor_done.set()
        store_thread = threading.Thread(target=store_pending)
        store_thread.start()
        assert entered.wait(10), 'provider did not receive accepted store'
        def barrier_pending():
            try:
                barrier_values.append(control.call('barrier', {'target': initial, 'deadlineMs': 30000}, sent=barrier_sent))
            finally:
                barrier_done.set()
        barrier_thread = threading.Thread(target=barrier_pending)
        barrier_thread.start()
        assert barrier_sent.wait(5)
        assert not barrier_done.wait(.1), 'barrier escaped accepted provider work'
        assert not actor_done.is_set()
        resume.set()
        store_thread.join(35)
        barrier_thread.join(35)
        assert actor_done.is_set() and barrier_done.is_set()
        assert len(stored_values) == len(barrier_values) == 1
        stored = stored_values[0]
        runtime_after_store = control.call('inspect', {})['result']['runtimeState']
        assert stored['createdAt'] == '2026-01-01 00:01:00 UTC', stored
        accepted_barrier = barrier_values[0]
        assert 'error' not in accepted_barrier, accepted_barrier
        assert accepted_barrier['result']['ready'] and accepted_barrier['result']['pendingWork'] == 0

        after_store = inspect()
        assert int(after_store['generation']) > int(initial['generation'])
        assert accepted_barrier['result']['committedGeneration'] == after_store['generation']
        barrier(after_store)
        before = files(root / 'data')
        result = tool('recall_memories', {'query': 'offline-known', 'recordAccess': False, 'namespace': 'default'})
        assert any(v['id'] == stored['id'] for v in result['memories']), result
        assert inspect() == after_store
        barrier(after_store)
        assert files(root / 'data') == before, 'false search/control byte change'
        tool('recall_memories', {'query': 'offline-known', 'recordAccess': True, 'namespace': 'default'})
        exposed = inspect()
        assert int(exposed['generation']) > int(after_store['generation'])
        barrier(exposed)
        tool('reinforce_memory', {'id': stored['id'], 'quality': 3, 'namespace': 'default'})
        reinforced = inspect()
        assert int(reinforced['generation']) > int(exposed['generation'])
        barrier(reinforced)
        before = files(root / 'data')
        tool('recall_memories', {'query': 'fixture-miss', 'recordAccess': False}, ok=False)
        assert inspect() == reinforced and files(root / 'data') == before
        for method in ['snapshot', 'reset', 'sweep']:
            assert control.call(method, {})['error']['code'] == 'UNSUPPORTED_CONTROL'
        for target in [dict(reinforced, storeId='00000000-0000-0000-0000-000000000000'),
                       dict(reinforced, generation=str(int(reinforced['generation']) + 1))]:
            assert 'error' in control.call('barrier', {'target': target, 'deadlineMs': 100})
            barrier(reinforced)  # rejection must reopen admission
        for generation in [1, '-1', '01', '+1', '9223372036854775808']:
            assert control.call('barrier', {'target': dict(reinforced, generation=generation), 'deadlineMs': 100})['error']['code'] == 'INVALID_PARAMS'
        for params in [{'target': reinforced, 'deadlineMs': 0}, {'target': reinforced, 'deadlineMs': 30001},
                       {'target': reinforced, 'deadlineMs': 100, 'unknown': True}]:
            assert control.call('barrier', params)['error']['code'] == 'INVALID_PARAMS'
        assert control.call('inspect', {'unknown': True})['error']['code'] == 'INVALID_PARAMS'
        assert files(root / 'data') == before
        started = time.monotonic()
        os.close(request_write)
        request_write = None
        child.wait(timeout=5)
        assert child.returncode == 0, child.returncode
        return {'replay': index, 'passed': True, 'initial': initial, 'afterStore': after_store,
                'afterExposure': exposed, 'afterFeedback': reinforced,
                'namespaceCreatedAt': namespace['createdAt'], 'storedId': stored['id'], 'runtimeInitial': runtime_initial['runtimeState'], 'runtimeAfterStore': runtime_after_store,
                'acceptedBarrierWaitedForStore': True, 'controlEofExitSeconds': time.monotonic() - started, 'command': command}
    finally:
        save(root / 'transcript.json', log)
        if child.poll() is None:
            child.kill()  # only this probe's child, on failed bounded shutdown
            child.wait(timeout=5)
        if request_write is not None:
            os.close(request_write)
        os.close(response_read)
        child.stdin.close()
        child.stdout.close()
        stderr.close()


def negative_launches(binary, output, endpoint):
    results = []
    for case in ['duplicate-fd', 'actor-stdin-fd', 'wrong-direction', 'non-fifo',
                 'existing-root', 'oversized-frame', 'unterminated-eof']:
        root = output / case
        root.mkdir()
        home = root / 'home'
        home.mkdir()
        config = root / 'config.toml'
        config.write_text(f'[embedding]\nprovider="openai"\nmodel_name="offline-known-v1"\n'
                          f'base_url="{endpoint}"\napi_key_env="RECALLD_EVAL_OFFLINE_KEY"\n'
                          'dimensions=4\n[decay]\ndisable_sweep=true\n')
        data = root / 'data'
        if case == 'existing-root':
            data.mkdir()
            (data / 'sentinel').write_bytes(b'must remain unchanged')
        before = files(data) if data.exists() else None
        rq, wq = os.pipe()
        rr, wr = os.pipe()
        owned = {rq, wq, rr, wr}
        control_read, control_write = rq, wr
        if case == 'duplicate-fd':
            control_write = control_read
        elif case == 'actor-stdin-fd':
            control_read = 0
        elif case == 'wrong-direction':
            control_read, control_write = wq, rr
        elif case == 'non-fifo':
            regular = root / 'regular-file'
            regular.write_bytes(b'not a pipe')
            control_read = os.open(regular, os.O_RDONLY)
            owned.add(control_read)
        command = [str(binary), '--config', str(config), '--data-root', str(data),
                   '--control-read-fd', str(control_read), '--control-write-fd', str(control_write)]
        inherited = tuple(sorted({fd for fd in [control_read, control_write] if fd >= 3}))
        with (root / 'child.log').open('wb') as stderr:
            child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                    stderr=stderr, pass_fds=inherited, bufsize=0,
                    env={'PATH':'/usr/local/bin:/usr/bin:/bin', 'HOME':str(home), 'TMPDIR':str(home),
                         'RECALLD_EVAL_OFFLINE_KEY':'offline-fixture'})
            try:
                if case in ['oversized-frame', 'unterminated-eof']:
                    os.close(rq); owned.remove(rq)
                    os.close(wr); owned.remove(wr)
                    transcript = []
                    control = JsonLines(rr, wq, transcript)
                    assert 'result' in control.call('capabilities', {})
                    raw = b'x' * (16 * 1024 + 1) if case == 'oversized-frame' else b'{"id":99,"method":"inspect","params":{}}'
                    (root / 'request.bin').write_bytes(raw)
                    pending = raw
                    while pending:
                        pending = pending[os.write(wq, pending):]
                    os.close(wq); owned.remove(wq)
                    reply = b''
                    deadline = time.monotonic() + 5
                    while b'\n' not in reply:
                        left = deadline - time.monotonic()
                        assert left > 0 and select.select([rr], [], [], left)[0]
                        chunk = os.read(rr, 65536)
                        assert chunk, 'missing framing rejection'
                        reply += chunk
                    (root / 'response.bin').write_bytes(reply)
                    decoded = json.loads(reply.split(b'\n', 1)[0])
                    assert decoded['id'] is None and decoded['error']['code'] == 'INVALID_REQUEST', decoded
                    child.wait(timeout=5)
                    assert child.returncode == 0
                    save(root / 'transcript.json', transcript)
                else:
                    child.wait(timeout=5)
                    assert child.returncode != 0
                    assert (files(data) if data.exists() else None) == before
                results.append({'case':case, 'passed':True, 'exitCode':child.returncode, 'command':command})
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=5)
                child.stdin.close()
                child.stdout.close()
                for fd in owned:
                    os.close(fd)
                save(output / 'constructor-negatives.json', results)
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    requests = []
    hold, entered, resume = threading.Event(), threading.Event(), threading.Event()

    class Embeddings(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            requests.append(body)
            values = body.get('input', [])
            if isinstance(values, str):
                values = [values]
            allowed = {'offline-known', 'offline-known\n\noffline payload'}
            ok = body.get('model') == 'offline-known-v1' and values and all(v in allowed for v in values)
            if ok and 'offline-known\n\noffline payload' in values and hold.is_set():
                hold.clear()
                entered.set()
                assert resume.wait(15), 'supervisor did not release fixture provider'
            self.send_response(200 if ok else 404)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({'data': [{'index': i, 'embedding': [1., 0., 0., 0.]} for i in range(len(values))], 'usage': {'prompt_tokens': 0, 'total_tokens': 0}} if ok else {'error': 'offline replay miss'}).encode())

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Embeddings)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        negatives = negative_launches(args.binary.resolve(), args.output, f'http://127.0.0.1:{server.server_port}/v1')
        runs = [episode(args.binary.resolve(), args.output, f'http://127.0.0.1:{server.server_port}/v1', n, hold, entered, resume) for n in range(1, 4)]
        projections = [{k: run[k] for k in ('initial', 'afterStore', 'afterExposure', 'afterFeedback', 'storedId', 'runtimeInitial', 'runtimeAfterStore')} for run in runs]
        assert projections[0] == projections[1] == projections[2], 'owned allocation/time projection drift'
        save(args.output / 'summary.json', {'claim': 'same-core frozen clock/allocation diagnostic; matching projections are not whole-state reset/replay; no v2/A6 completion',
              'binarySha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'episodes': runs, 'constructorNegatives': negatives})
    finally:
        save(args.output / 'embedding-requests.json', requests)
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
