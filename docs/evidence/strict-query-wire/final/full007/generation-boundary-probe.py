import copy,json
from pathlib import Path
from jsonschema import Draft202012Validator,FormatChecker
root=Path('/output/schema-fixtures')
f=json.loads((root/'recall-plain.json').read_text());v=Draft202012Validator(f['schema'],format_checker=FormatChecker())
for value in ['1','1\n','1\r','1\r\n','1\u2028','1\u2029']:
 x=copy.deepcopy(f['value']);x['readGeneration']['generation']=value
 print('generation',repr(value),'schema_accepts=',v.is_valid(x))
inputs=json.loads((root/'input-cases.json').read_text())
s=next(c['schema'] for c in inputs if 'queryMode' in c['schema']['properties'])
v=Draft202012Validator(s)
for suffix in ['', '\n', '\r', '\u2028','\u2029']:
 x={'contractVersion':2,'embedding':[1], 'embeddingProfileId':'ep1:'+'a'*64+suffix}
 print('profile_suffix',repr(suffix),'schema_accepts=',v.is_valid(x))
