#!/usr/bin/env python3
"""Validate the actual frozen serialized MCP consumer; no builds or mock handlers."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir()
    source = args.root / 'source'
    inputs = args.root / 'output/inputs'
    baseline = args.root / 'output/episode-0'
    binary, scorer = args.root / 'target/debug/recalld', args.root / 'target/debug/examples/score_mcp_episode'
    script = source / 'scripts/public-mcp-evaluation.py'
    manifest = json.loads((inputs / 'manifest.json').read_text())
    def command(root, output):
        return [sys.executable, str(script), 'run', '--manifest', str(root / 'manifest.json'), '--binary', str(binary), '--scorer', str(scorer), '--source-root', str(source), '--out', str(output)]
    def case_inputs(name, change):
        root = args.out / name; shutil.copytree(inputs, root)
        m = copy.deepcopy(manifest)
        actor = json.loads((root / 'actor-inputs.json').read_text())
        labels = json.loads((root / 'evaluator-labels.json').read_text())
        cache = json.loads((root / 'evaluator-cache.json').read_text())
        change(m, actor, labels, cache)
        for key, filename, value in [('actorInputs','actor-inputs.json',actor), ('evaluatorLabels','evaluator-labels.json',labels), ('replayCache','evaluator-cache.json',cache)]:
            save(root / filename, value); m[key]['sha256'] = sha(root / filename)
        m['dataset']['actorArtifactSha256'] = m['actorInputs']['sha256']
        save(root / 'manifest.json', m)
        return root
    report = dict(status='running', sourceCommit=manifest['sourceCommit'], binarySha256=sha(binary), scorerSha256=sha(scorer), checkerSha256=sha(Path(__file__)), rejections=[])
    try:
        cases = [(f'version-{field}', lambda m,a,e,c,field=field:m.update({field:2})) for field in ('manifestVersion','sourceMappingVersion','traceSchemaVersion')]
        cases += [(f'capability-{field}', lambda m,a,e,c,field=field:m['capabilities'].update({field:True})) for field in ('publicV2','fullA6','reset','barrier','deterministicRng','clockAdvance')]
        cases += [
            ('invented-native-schema',lambda m,a,e,c:m['versions'].update(canonicalSchemaVersion=2)),
            ('forged-source',lambda m,a,e,c:m.update(sourceCommit='0'*40)),
            ('forged-binary',lambda m,a,e,c:m['executables'].update(recalld='0'*64)),
            ('future-schedule-in-actor',lambda m,a,e,c:a.update(queries=[{'text':'FUTURE_LEAK'}])),
            ('gold-label-in-actor',lambda m,a,e,c:a.update(answer=e['labelCanary'])),
            ('unknown-label',lambda m,a,e,c:e['relevant'].update(q0=['foreign-source'])),
            ('duplicate-label',lambda m,a,e,c:e['relevant'].update(q0=['turn-0','turn-0'])),
            ('cache-miss-generation',lambda m,a,e,c:c['entries'].update({'a6-query:unrecorded-query':[1.,0.,0.,0.]})),
        ]
        for name, change in cases:
            root = case_inputs('reject-' + name, change); output = root / 'must-not-exist'
            run = subprocess.run(command(root, output), capture_output=True, text=True)
            report['rejections'].append(dict(case=name, exitCode=run.returncode, stdout=run.stdout, stderr=run.stderr, outputCreated=output.exists()))
            save(args.out / 'report.json', report)
            assert run.returncode != 0 and not output.exists(), name
        def perturb(m,a,e,c):
            e['labelCanary'] = 'CHANGED_GOLD_LABEL_CANARY'
            e['relevant'] = {f'q{i}':['turn-3'] for i in range(4)}
        variant_inputs = case_inputs('label-perturbation-inputs', perturb)
        variant = args.out / 'label-perturbation-episode'
        run = subprocess.run(command(variant_inputs, variant), capture_output=True, text=True)
        save(args.out / 'label-run.json', dict(exitCode=run.returncode, stdout=run.stdout, stderr=run.stderr))
        assert run.returncode == 0
        assert (baseline / 'actor-projection.json').read_bytes() == (variant / 'actor-projection.json').read_bytes()
        assert (baseline / 'actor-code.py').read_bytes() == (variant / 'actor-code.py').read_bytes()
        for i in range(6):
            assert (baseline / f'actor-inputs/step-{i}.json').read_bytes() == (variant / f'actor-inputs/step-{i}.json').read_bytes()
        before, after = rows(baseline / 'actor-wire.jsonl'), rows(variant / 'actor-wire.jsonl')
        old_requests = [r['request'] for r in before if r['event'] == 'attempt']
        new_requests = [r['request'] for r in after if r['event'] == 'attempt']
        assert old_requests == new_requests and len(old_requests) == 15
        old_provider, new_provider = rows(baseline / 'actor-provider.jsonl'), rows(variant / 'actor-provider.jsonl')
        assert [(r['requestBody'],r['status']) for r in old_provider] == [(r['requestBody'],r['status']) for r in new_provider]
        assert all(a['cacheKeys'] != b['cacheKeys'] for a,b in zip(old_provider,new_provider))
        old_scores, new_scores = [json.loads((p / 'evaluator/scores.json').read_text()) for p in (baseline, variant)]
        assert old_scores['scoring']['scored'] == '2' and old_scores['scoring']['unknownAnnotations'] == '1'
        assert old_scores['scoring']['zeroLabelUnscored'] == '1'
        assert old_scores['scoring']['means']['retrievalRecallAt5'] == .75
        assert new_scores['scoring']['scored'] == '4' and new_scores['scoring']['means']['retrievalRecallAt5'] == 0.
        assert [r['retrievedSourceIds'] for r in old_scores['trace']] == [r['retrievedSourceIds'] for r in new_scores['trace']] == [['turn-0']]*4
        for directory, gold_canary in ((baseline,'EVALUATOR_LABEL_CANARY_ONLY'), (variant,'CHANGED_GOLD_LABEL_CANARY')):
            assert json.loads((directory / 'result.json').read_text())['futureQueryCanaryWithheldUntilDelivery'] is True
            for i in range(4): assert 'FUTURE_QUESTION_CANARY' not in (directory / f'actor-inputs/step-{i}.json').read_text()
            assert 'FUTURE_QUESTION_CANARY' in (directory / 'actor-inputs/step-4.json').read_text()
            for name in ('actor-code.py','actor-projection.json','actor-wire.jsonl','actor-provider.jsonl','trace.jsonl'):
                assert gold_canary not in (directory / name).read_text()
            for row in rows(directory / 'actor-wire.jsonl') + rows(directory / 'actor-provider.jsonl'):
                if row['deliveryOrdinal'] < 4: assert 'FUTURE_QUESTION_CANARY' not in json.dumps(row)
        report.update(status='passed', actorRequestsCompared=15, deliveredProjectionsCompared=6,
            providerRequestsCompared=len(old_provider), evaluatorMetricsChanged=True, actorCodeAndInputsUnchanged=True,
            futureQuestionWithheldUntilCurrentDelivery=True, returnedSourceRanksUnchanged=True,
            comparison='Raw UUIDs/timestamps retained; returned-source ranks compared through actual per-run mappings. No deterministic-reset or byte-identical public-result claim.')
    finally:
        report['artifactsSha256'] = {str(p.relative_to(args.out)):sha(p) for p in sorted(args.out.rglob('*')) if p.is_file() and p.name!='report.json'}
        save(args.out / 'report.json', report)
    print(json.dumps({k:v for k,v in report.items() if k not in ('artifactsSha256','rejections')},sort_keys=True))


if __name__ == '__main__':
    main()
