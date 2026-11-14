#!/usr/bin/env python3
"""Run frozen-manifest rejection goldens through the actual compiled consumer."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    baseline = json.loads(args.manifest.read_text())
    binary_sha256 = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    cases = []
    for field in ('manifestVersion', 'sourceMappingVersion', 'traceSchemaVersion'):
        cases.append((field, lambda m, a, e, field=field: m.update({field: 2})))
    for field in ('contractVersion', 'canonicalSchemaVersion', 'recordSchemaVersion', 'receiptVersion'):
        cases.append((field, lambda m, a, e, field=field: m['versions'].update({field: 2 if field != 'receiptVersion' else 99})))
    for field in ('publicV2', 'nativeV2', 'fullA6', 'barrier', 'snapshot', 'reset', 'deterministicIds', 'deterministicRng', 'clockAdvance'):
        cases.append((field, lambda m, a, e, field=field: m['capabilities'].update({field: True})))
    cases += [
        ('unused-backup-requirement', lambda m, a, e: m['versions'].update({'backupFormatVersion': 2})),
        ('unused-export-requirement', lambda m, a, e: m['versions'].update({'exportFormatVersion': 1})),
        ('invented-canonical-zero', lambda m, a, e: m['versions'].update({'canonicalSchemaVersion': 0})),
        ('invented-canonical-one', lambda m, a, e: m['versions'].update({'canonicalSchemaVersion': 1})),
        ('native-representation', lambda m, a, e: m['versions'].update({'canonicalRepresentation': 'native-v2'})),
        ('missing-version', lambda m, a, e: m.pop('traceSchemaVersion')),
        ('unknown-field', lambda m, a, e: m.update({'futureCapability': True})),
        ('forged-resolved-revision', lambda m, a, e: m.update({'sourceCommit': '0' * 40})),
        ('forged-image', lambda m, a, e: m.update({'containerImageId': 'sha256:' + '0' * 64})),
        ('forged-dirty-patch', lambda m, a, e: m.update({'dirtyPatchSha256': '0' * 64})),
        ('unresolved-revision', lambda m, a, e: m.update({'sourceCommit': 'unresolved'})),
        ('external-dataset', lambda m, a, e: m['dataset'].update({'kind': 'upstream'})),
        ('unverified-license', lambda m, a, e: m['dataset'].update({'thirdPartyLicense': 'invented'})),
        ('artifact-traversal', lambda m, a, e: m['actorInputs'].update({'file': '../actor-inputs.json'})),
        ('actor-hash', lambda m, a, e: m['actorInputs'].update({'sha256': '0' * 64})),
        ('omitted-runtime-source', lambda m, a, e: m['sourceFilesSha256'].pop('src/lib.rs')),
        ('source-hash', lambda m, a, e: m['sourceFilesSha256'].update({'Cargo.lock': '0' * 64})),
        ('actor-label-injection', lambda m, a, e: a['sources'][0].update({'answer': e['labelCanary']})),
        ('duplicate-source', lambda m, a, e: a['sources'].__setitem__(1, copy.deepcopy(a['sources'][0]))),
        ('content-hash', lambda m, a, e: a['sources'][0].update({'contentSha256': '0' * 64})),
        ('unknown-evidence', lambda m, a, e: e['relevant'].update({'query0': ['absent']})),
        ('missing-query', lambda m, a, e: e['relevant'].pop('query0')),
        ('live-fallback', lambda m, a, e: m['embedding'].update({'cacheMiss': 'generate'})),
        ('wrong-role', lambda m, a, e: m['embedding'].update({'role': 'judge'})),
        ('bad-resource-pin', lambda m, a, e: m['limits'].update({'cpus': 4})),
    ]
    results = []
    for name, change in cases:
        with tempfile.TemporaryDirectory(prefix='manifest-rejection-') as temporary:
            root = Path(temporary)
            m = copy.deepcopy(baseline)
            a = json.loads((args.manifest.parent / 'actor-inputs.json').read_text())
            e = json.loads((args.manifest.parent / 'evaluator-labels.json').read_text())
            change(m, a, e)
            # Content mutations have correct outer hashes, so the semantic boundary is exercised.
            for field, filename, value in [('actorInputs', 'actor-inputs.json', a), ('evaluatorLabels', 'evaluator-labels.json', e)]:
                data = (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()
                (root / filename).write_bytes(data)
                if name != 'actor-hash':
                    m[field]['sha256'] = hashlib.sha256(data).hexdigest()
            m['dataset']['datasetRevision'] = m['actorInputs']['sha256']
            (root / 'manifest.json').write_text(json.dumps(m))
            output = root / 'must-not-exist'
            run = subprocess.run([str(args.binary), '--manifest', str(root / 'manifest.json'), '--out', str(output)],
                                 capture_output=True, text=True)
            result = {'case': name, 'exitCode': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr,
                      'outputCreated': output.exists(), 'binarySha256': binary_sha256,
                      'manifestJson': (root / 'manifest.json').read_text(),
                      'actorInputsJson': (root / 'actor-inputs.json').read_text(),
                      'evaluatorLabelsJson': (root / 'evaluator-labels.json').read_text(),
                      'manifestSha256': hashlib.sha256((root / 'manifest.json').read_bytes()).hexdigest()}
            results.append(result)
            # Retain partial results even if an assertion fails.
            args.output.write_text(json.dumps(results, indent=2) + '\n')
            if run.returncode == 0 or output.exists():
                raise RuntimeError(f'failed closed preflight golden: {name}: {result}')
    print(f'{len(results)} actual-consumer preflight rejections; no episode output/store created')


if __name__ == '__main__':
    main()
