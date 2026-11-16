"""Consumed paired synthetic cohorts; both arms validate before any episode starts."""
import json
import subprocess
from types import SimpleNamespace

TRACK = 'serialized-mcp-paired-grouped-synthetic-diagnostic-v1'
ARM_TRACK = 'serialized-mcp-paired-grouped-arm-v1'
PROFILE = 'synthetic-paired-grouped-unique-source-v1'
POLICY = dict(uncertaintyVersion=1, replicates=10000, seed=7,
    rng='rand_chacha-0.3.1-ChaCha12Rng', seedEncoding='u64-le-zero-pad32', boundedIndex='rand-0.8.6-u64-gen-range',
    streams=dict(train=0, development=1, test=2), interval='nearest-rank-2.5-97.5', minimumEligibleGroups=2,
    contrast='candidate-minus-baseline', population='whole-history-groups-with-common-scored-queries')
ROLES = dict(baseline=1, candidate=2)


def fixture(episode, h):
    actor, labels, cache, schedule = h['GROUPED']['episode_fixture'](episode, h)
    index = int(episode['episodeId'].rsplit('-', 1)[1])
    q0, q3 = (([0,1],[1]),([1],[3]),([0],[1]),([0,1],[0,3]),([0,3],[1]),([0],[0]),([1],[0,1]))[index]
    labels['relevant'] = dict(q0=[f'turn-{i}' for i in q0], q1=[], q3=[f'turn-{i}' for i in q3])
    cache['entries']['a6-document:' + actor['sources'][1]['summary']] = [.8,.6,0.,0.]
    return actor, labels, cache, schedule


def contract(h):
    g = h['GROUPED']; sources = g['registry'](h); groups, splits = g['maps'](sources, h)
    # Groups freeze before constructing any labels or embedding-cache artifacts.
    episodes = {e['episodeId']: {key: h['digest'](h['encoded'](value)) for (key,_),value in zip(g['ARTIFACTS'],fixture(e,h))}
                for e in sources['episodes']}
    return dict(pairContractVersion=1, fixture='paired-public-limit-synthetic-v1', uncertaintyPolicy=POLICY,
        armPolicies={role:dict(retrievalLimit=limit,recordAccess=False,depth=0,namespace='default') for role,limit in ROLES.items()},
        sourceRegistrySha256=h['digest'](h['encoded'](sources)), groupMapSha256=h['digest'](h['encoded'](groups)),
        splitMapSha256=h['digest'](h['encoded'](splits)), episodes=episodes)


def arm_context(role, sha):
    return dict(role=role, retrievalLimit=ROLES[role], pairContractSha256=sha, fixtureVersion=1)


def prepare(args,h):
    g=h['GROUPED']; shared=contract(h); build=json.loads(args.build.read_text());m=h['build_manifest'](args,build)
    m.update(track=TRACK,pairVersion=1,comparisonProfile=PROFILE,uncertaintyPolicy=POLICY)
    args.out.mkdir();m['pairContract']=g['reference'](args.out,'pair-contract.json',shared,h);m['arms']=[]
    for role in ROLES:
        child=SimpleNamespace(**vars(args));child.out=args.out/role
        g['prepare'](child,h,paired=arm_context(role,m['pairContract']['sha256']))
        path=child.out/'manifest.json';m['arms'].append(dict(role=role,manifest=dict(file=role+'/manifest.json',sha256=h['digest'](path.read_bytes()))))
    m['dataset']=dict(kind='generated-synthetic-paired-grouped',upstreamRevision=None,upstreamUrl=None,thirdPartyLicense=None,
        pairContractSha256=m['pairContract']['sha256'],intendedUse='contract-diagnostic-only')
    h['save'](args.out/'manifest.json',m)


def validate(args,h):
    g=h['GROUPED'];raw=args.manifest.read_bytes();m=json.loads(raw);root=args.manifest.parent
    h['exact_keys'](m,(*h['COMMON_FIELDS'],'pairVersion','comparisonProfile','uncertaintyPolicy','pairContract','arms','dataset'))
    if m['track']!=TRACK or type(m['pairVersion']) is not int or m['pairVersion']!=1 or m['comparisonProfile']!=PROFILE or h['encoded'](m['uncertaintyPolicy'])!=h['encoded'](POLICY):
        raise ValueError('unsupported paired policy/version')
    h['validate_build'](m,args.binary,args.scorer,args.source_root)
    shared=g['load'](root,m['pairContract'],'pair-contract.json',h)
    if h['encoded'](shared)!=h['encoded'](contract(h)):raise ValueError('paired contract differs from frozen source/label/policy fixture')
    if m['dataset']!=dict(kind='generated-synthetic-paired-grouped',upstreamRevision=None,upstreamUrl=None,thirdPartyLicense=None,
        pairContractSha256=m['pairContract']['sha256'],intendedUse='contract-diagnostic-only'):raise ValueError('unsupported paired dataset claim')
    if not isinstance(m['arms'],list) or len(m['arms'])!=2 or {r.get('role') for r in m['arms']}!=set(ROLES):raise ValueError('incomplete/duplicate paired arm roster')
    validated={}
    for entry in sorted(m['arms'],key=lambda x:x['role']):
        h['exact_keys'](entry,('role','manifest'));role=entry['role'];arm=g['load'](root,entry['manifest'],role+'/manifest.json',h)
        if any(h['encoded'](arm[k])!=h['encoded'](m[k]) for k in h['COMMON_FIELDS'] if k!='track'):raise ValueError('paired build/profile differs')
        child=SimpleNamespace(**vars(args));child.manifest=root/role/'manifest.json';child.out=args.out/role
        value=g['validate'](child,h,paired=arm_context(role,m['pairContract']['sha256']))
        for key in ('sourceRegistry','groupMap','splitMap'):
            if arm[key]['sha256']!=shared[key+'Sha256']:raise ValueError('paired source/group/split differs')
        for member in arm['episodes']:
            for key,_ in g['ARTIFACTS']:
                if member['artifacts'][key]['sha256']!=shared['episodes'][member['episodeId']][key]:raise ValueError('paired shared source/labels/query/cache differs')
        validated[role]=(child,value)
    if validated['baseline'][1][2]!=validated['candidate'][1][2]:raise ValueError('paired member/query roster differs')
    return m,validated,h['digest'](raw)


def run(args,h):
    m,validated,environment=validate(args,h)  # Both full arms before output/provider/store.
    args.out.mkdir();status=dict(status='running',environmentHash=environment,plannedMembers=14,
        arms={role:dict(status='unattempted') for role in ROLES},members=[dict(role=role,**member,status='unattempted') for role in ROLES for member in validated[role][1][2]])
    def record():
        for role in ROLES:
            path=args.out/role/'episode-status.json'
            if path.exists():
                child=json.loads(path.read_text());by_id={row['episodeId']:row['status'] for row in child['episodes']}
                for member in status['members']:
                    if member['role']==role:member['status']=by_id[member['episodeId']]
        for key,state in (('passedMembers','passed'),('failedMembers','failed'),('unattemptedMembers','unattempted'),('runningMembers','attempted')):
            status[key]=sum(v['status']==state for v in status['members'])
        status['attemptedMembers']=status['plannedMembers']-status['unattemptedMembers']
        h['save'](args.out/'pair-status.json',status)
    record()
    try:
        for role in ROLES:
            status['arms'][role]['status']='running';record()
            try:h['GROUPED']['run'](validated[role][0],h,prepared=validated[role][1])
            except BaseException:
                status['arms'][role]['status']='failed';raise
            status['arms'][role]['status']='passed';record()
        root=args.manifest.parent;pair_raw=args.manifest.read_text();contract_raw=(root/'pair-contract.json').read_text()
        score=dict(profile=PROFILE,pairManifestJson=pair_raw,pairManifestSha256=h['digest'](pair_raw.encode()),
            contractJson=contract_raw,contractSha256=h['digest'](contract_raw.encode()),arms=[])
        for role in ROLES:
            arm=dict(role=role)
            for key,path in (('manifest',root/role/'manifest.json'),('groupedInput',args.out/role/'grouped-score-input.json'),('status',args.out/role/'episode-status.json')):
                raw=path.read_text();arm[key+'Json']=raw;arm[key+'Sha256']=h['digest'](raw.encode())
            score['arms'].append(arm)
        h['save'](args.out/'paired-score-input.json',score)
        subprocess.run([str(args.scorer),'--paired-input',str(args.out/'paired-score-input.json'),'--out',str(args.out/'paired-scores.json')],check=True,env=dict(PATH='/usr/bin:/bin'))
        status['status']='passed'
    except BaseException as error:status.update(status='failed',error=str(error));raise
    finally:record()
