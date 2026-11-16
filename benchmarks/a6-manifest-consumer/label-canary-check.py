#!/usr/bin/env python3
"""Actual frozen-consumer label perturbation; no Rust build or model calls."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def key(manifest, manifest_bytes, text):
    payload = {'namespace': 'fixture', 'text': text, 'role': 'actor-query',
               'profile': manifest['embedding'], 'environmentHash': digest(manifest_bytes)}
    return digest(json.dumps(payload, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir()
    original_bytes = args.manifest.read_bytes()
    original = json.loads(original_bytes)
    manifest = json.loads(original_bytes)
    inputs = args.out / 'inputs'
    inputs.mkdir()
    actor = (args.manifest.parent / 'actor-inputs.json').read_bytes()
    (inputs / 'actor-inputs.json').write_bytes(actor)
    labels = json.loads((args.manifest.parent / 'evaluator-labels.json').read_text())
    # Valid existing source in the distractor namespace; all fixture queries stay unchanged.
    labels['labelCanary'] = 'PERTURBED_EVALUATOR_ONLY_CANARY_NEVER_ACTOR'
    last_source = json.loads(actor)['sources'][-1]['sourceId']
    labels['relevant'] = {q: [last_source] for q in labels['relevant']}
    save(inputs / 'evaluator-labels.json', labels)
    manifest['evaluatorLabels']['sha256'] = digest((inputs / 'evaluator-labels.json').read_bytes())
    save(inputs / 'manifest.json', manifest)
    changed_bytes = (inputs / 'manifest.json').read_bytes()
    report = {'kind': 'actual-synthetic-label-perturbation', 'status': 'running',
              'sourceCommit': original['sourceCommit'], 'checkerSha256': digest(Path(__file__).read_bytes()),
              'binarySha256': digest(args.binary.read_bytes()),
              'baselineManifestSha256': digest(original_bytes), 'perturbedManifestSha256': digest(changed_bytes),
              'changes': ['evaluator labels', 'evaluator canary', 'evaluator artifact hash'],
              'semanticQualityClaim': False, 'deterministicResetClaim': False}
    save(args.out / 'report.json', report)
    try:
        assert manifest['actorInputs'] == original['actorInputs']
        assert digest(original_bytes) != digest(changed_bytes)
        command = [str(args.binary), '--manifest', str(inputs / 'manifest.json'), '--out', str(args.out / 'run')]
        report['command'] = command
        with (args.out / 'consumer.log').open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        report['consumerExitCode'] = result.returncode
        assert result.returncode == 0, 'actual perturbed consumer failed; raw log retained'
        changed = args.out / 'run'
        stable = ['fixture.json', 'actor-inputs.json', 'state-before.json', 'state-after.json',
                  'replay.json', 'source-mappings.jsonl']
        for name in stable:
            assert (args.baseline / name).read_bytes() == (changed / name).read_bytes(), name
        before, after = rows(args.baseline / 'trace.jsonl'), rows(changed / 'trace.jsonl')
        assert len(before) == len(after)
        for a, b in zip(before, after):
            assert a['stepId'] == b['stepId']
            # Cache identity is explicitly permitted to change; no action content is normalized.
            left, right = dict(a['action']), dict(b['action'])
            old_key, new_key = left.pop('replayCacheKey', None), right.pop('replayCacheKey', None)
            assert left == right, a['stepId']
            if old_key is not None:
                assert new_key is not None and old_key != new_key
        old_cache = json.loads((args.baseline / 'actor-replay-cache.json').read_text())
        new_cache = json.loads((changed / 'actor-replay-cache.json').read_text())
        assert old_cache['environmentHash'] != new_cache['environmentHash']
        assert old_cache['profile'] == new_cache['profile']
        assert len(old_cache['entries']) == len(new_cache['entries']) == len(labels['relevant'])
        for query in labels['relevant']:
            old_key, new_key = key(original, original_bytes, query), key(manifest, changed_bytes, query)
            assert old_key != new_key
            assert old_cache['entries'][old_key] == new_cache['entries'][new_key], query
        for name in stable + ['trace.jsonl', 'actor-replay-cache.json']:
            assert labels['labelCanary'].encode() not in (changed / name).read_bytes(), name
        old_metrics = [r['metrics'] for r in rows(args.baseline / 'evaluator/trace.jsonl') if 'metrics' in r]
        new_metrics = [r['metrics'] for r in rows(changed / 'evaluator/trace.jsonl') if 'metrics' in r]
        assert old_metrics and len(old_metrics) == len(new_metrics)
        assert all(r['retrievalRecallAt5'] == 1.0 for r in old_metrics)
        assert all(r['retrievalRecallAt5'] == 0.0 for r in new_metrics)
        # Reuse PR75's exact receipt comparison, with only its already declared store/operation-ID exceptions.
        comparison = args.out / 'receipt-comparison'
        for index, origin in enumerate([args.baseline, changed]):
            destination = comparison / f'run-{index}'
            destination.mkdir(parents=True)
            for name in ['fixture.json', 'state-before.json', 'state-after.json', 'replay.json',
                         'control-before.json', 'control-after.json', 'turn-ledger.jsonl']:
                shutil.copyfile(origin / name, destination / name)
        runner = runpy.run_path('/work/source/scripts/offline-evaluation.py')
        runner['verify_replays'](comparison, 2)
        report.update(status='passed', actorActionsCompared=len(before), replayVectorsCompared=len(labels['relevant']),
                      byteIdenticalArtifacts=stable, evaluatorMetricPairs=len(old_metrics),
                      evaluatorMetricsChanged=True, originalReceiptSemanticsVerified=True,
                      cacheIdentityChanged=True, actorBehaviorChanged=False)
    except BaseException as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        report['artifactsSha256'] = {str(p.relative_to(args.out)): digest(p.read_bytes())
                                    for p in sorted(args.out.rglob('*')) if p.is_file() and p.name != 'report.json'}
        save(args.out / 'report.json', report)
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
