#!/usr/bin/env python3
import json,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
inv=json.loads((root/'source-inventory/inventory.json').read_text())
idx=json.loads((root/'source-inventory/fixtures/contract-index.json').read_text())
errors=[]
if idx.get('source_count') != len(inv['sources']): errors.append('contract source_count mismatch')
paths=idx.get('contracts',[])
if len(paths)!=len(set(paths)): errors.append('duplicate contract paths')
for source,path in zip(inv['sources'],paths):
 p=root/path
 if not p.is_file(): errors.append(f'missing {path}'); continue
 c=json.loads(p.read_text())
 if c.get('source_id')!=source['id']: errors.append(f'{path}: source order/id mismatch')
 if c.get('configuration')!=source['configuration']: errors.append(f'{path}: configuration drift')
 expected=[{'observed_count':o.get('count'),'first_title':o.get('first_title'),'first_url':o.get('first_url'),'warning':o.get('warning',''),'evidence_file':o.get('evidence_file')} for o in source.get('historical_observations',[])]
 if c.get('historical_expectations')!=expected: errors.append(f'{path}: observation drift')
 if 'not_a_raw_http_response' not in c.get('limitations',[]): errors.append(f'{path}: missing provenance limitation')
if errors:
 print('\n'.join(errors),file=sys.stderr); raise SystemExit(1)
print(f'source evidence contracts valid: {len(paths)}/{len(inv["sources"])}')
