"""Pinned local tokenizer and actor-owned renderer; no fixture/label/schedule imports."""
import base64
import hashlib
import json
from pathlib import Path
import platform
import sys

HERE=Path(__file__).parent
PROMPT='Use only the following evidence. If it is insufficient, abstain.'
PROFILES={'default':(4096,4608),'boundary':(128,256)}


def encoded(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
def digest(data):return hashlib.sha256(data).hexdigest()
def utf8(text):return text.encode('utf-8','strict')
def lock():return json.loads((HERE/'context-tokenizer-lock.json').read_text())

def identity():
    value=lock()
    return dict(library=value['library'],version=value['version'],encoding=value['encoding'],upstreamRevision=value['upstreamRevision'],
                lockSha256=digest((HERE/'context-tokenizer-lock.json').read_bytes()),rankSha256=value['rankSha256'],configSha256=value['configSha256'])

def policy(profile):
    if profile not in PROFILES:raise ValueError('unsupported context profile')
    evidence,total=PROFILES[profile]
    return dict(contextVersion=1,profile=profile,evidenceTokenCap=evidence,renderedContextTokenCap=total,renderer='ranked-whole-prefix-v1',
                promptSha256=digest(utf8(PROMPT)),utf8='strict',specialTokens='ordinary-text',tokenizer=identity())

def validate_policy(value):
    if not isinstance(value,dict) or encoded(value)!=encoded(policy(value.get('profile'))):raise ValueError('unsupported context policy')

def verify_bundle(bundle):
    bundle=Path(bundle);expected=lock();site=bundle/'site'
    if site.is_symlink():raise ValueError('symlinked tokenizer site rejected')
    files={str(p.relative_to(site)):p for p in site.rglob('*') if p.is_file() or p.is_symlink()}
    if set(files)!=set(expected['installedFilesSha256']):raise ValueError('installed tokenizer file roster mismatch')
    for name,p in files.items():
        if p.is_symlink() or not p.resolve().is_relative_to(site.resolve()) or digest(p.read_bytes())!=expected['installedFilesSha256'][name]:raise ValueError('installed tokenizer file hash mismatch')
    for name,key in [('cl100k_base.tiktoken','rankSha256'),('context-encoding.json','configSha256')]:
        p=bundle/name
        if not p.is_file() or p.is_symlink() or digest(p.read_bytes())!=expected[key]:raise ValueError('tokenizer asset/config hash mismatch')
    return expected

def load(bundle):
    bundle=Path(bundle);expected=verify_bundle(bundle)
    if f'{sys.version_info.major}.{sys.version_info.minor}'!=expected['python'] or platform.machine()!=expected['architecture']:raise ValueError('unsupported tokenizer wheel ABI')
    sys.dont_write_bytecode=True
    sys.path.insert(0,str(bundle/'site'))
    import tiktoken
    import tiktoken._tiktoken as native
    for module in (tiktoken,native):
        if not Path(module.__file__).resolve().is_relative_to((bundle/'site').resolve()):raise ValueError('ambient tokenizer module rejected')
    if tiktoken.__version__!=expected['version']:raise ValueError('installed tokenizer version mismatch')
    cfg=json.loads((bundle/'context-encoding.json').read_text());ranks={}
    for line in (bundle/'cl100k_base.tiktoken').read_bytes().splitlines():
        token,rank=line.split();key=base64.b64decode(token,validate=True)
        if key in ranks:raise ValueError('duplicate tokenizer rank bytes')
        ranks[key]=int(rank)
    return tiktoken.Encoding(**cfg,mergeable_ranks=ranks)

def tokens(encoding,text):
    utf8(text)
    return encoding.encode_ordinary(text)

def entry_text(memory,rank):
    if set(memory)!={'id','summary'} or not isinstance(memory['id'],str) or not isinstance(memory['summary'],str):raise ValueError('unsupported public context memory')
    # JSON escaping makes record boundaries unambiguous without altering the summary.
    utf8(memory['summary']);utf8(memory['id'])
    return json.dumps(dict(rank=rank,id=memory['id'],summary=memory['summary']),ensure_ascii=False,sort_keys=True,separators=(',',':'))+'\n'

def render(question,block):
    utf8(question)
    return PROMPT+'\nQuestion: '+json.dumps(question,ensure_ascii=False)+'\nEvidence:\n'+block+'Answer:\n'

def overhead(encoding,question,configuration):
    validate_policy(configuration);text=render(question,'');ids=tokens(encoding,text)
    if len(ids)>configuration['renderedContextTokenCap']:raise ValueError('fixed prompt/question overhead exceeds full context cap')
    return text,ids

def assemble(encoding,question,memories,configuration):
    empty,base_ids=overhead(encoding,question,configuration)
    block='';considered=[];included=[];omitted=[];entries=[];seen=set();stopped=False
    for rank,memory in enumerate(memories,1):
        text=entry_text(memory,rank);identity=memory['id']
        if identity in seen:raise ValueError('duplicate public memory identity')
        seen.add(identity);considered.append(identity)
        row=dict(id=identity,rank=rank,summarySha256=digest(utf8(memory['summary'])),renderedEntry=text,entrySha256=digest(utf8(text)))
        candidate=block+text
        fits=(len(tokens(encoding,candidate))<=configuration['evidenceTokenCap'] and len(tokens(encoding,render(question,candidate)))<=configuration['renderedContextTokenCap'])
        if not stopped and fits:
            block=candidate;included.append(identity);row.update(status='included',reason='ranked-prefix-fits')
        else:
            reason='after-first-nonfitting-entry' if stopped else 'context-budget';stopped=True;omitted.append(identity);row.update(status='omitted',reason=reason)
        entries.append(row)
    text=render(question,block);ids=tokens(encoding,text);evidence_ids=tokens(encoding,block)
    if len(ids)>configuration['renderedContextTokenCap'] or len(evidence_ids)>configuration['evidenceTokenCap']:raise ValueError('rendered context overflow')
    return dict(contextVersion=1,policy=configuration,questionSha256=digest(utf8(question)),renderedText=text,renderedSha256=digest(utf8(text)),
        tokenIds=ids,tokenIdsSha256=digest(encoded(ids)),fullTokenCount=len(ids),evidenceTokenCount=len(evidence_ids),
        emptyEvidenceOverheadTokenCount=len(base_ids),countsAreAdditive=False,consideredMemoryIds=considered,includedMemoryIds=included,
        omittedMemoryIds=omitted,entries=entries,encodingSpecificOnly=True,modelTokenIdentity=None)

def verify(encoding,question,public_memories,configuration,receipt):
    # Reconstruct from the actual public payload, never actor-supplied entry text/IDs.
    expected=assemble(encoding,question,public_memories,configuration)
    if encoded(receipt)!=encoded(expected):raise ValueError('actor rendered context differs from actual public payload/policy')
    return expected
