#!/usr/bin/env python3
"""Build deterministic regression contracts from preserved evidence, never live data."""
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
INV=ROOT/'source-inventory/inventory.json'
OUT=ROOT/'source-inventory/fixtures/contracts'
LEGACY={'cloudera','digoal','mirrorship','pingkai','modb-news','infoq-bigdata','highgo'}

def main():
 data=json.loads(INV.read_text())
 OUT.mkdir(parents=True,exist_ok=True)
 names=[]
 for source in data['sources']:
  sid=source['id']; obs=source.get('historical_observations',[])
  expectation=[]
  for item in obs:
   expectation.append({'observed_count':item.get('count'),'first_title':item.get('first_title'),'first_url':item.get('first_url'),'warning':item.get('warning',''),'evidence_file':item.get('evidence_file')})
  provenance='ported_legacy_inline_fixture_and_historical_observation' if sid in LEGACY else ('preserved_historical_observation' if obs else 'configuration_only_no_observed_result')
  payload={'schema_version':1,'source_id':sid,'provenance':provenance,'configuration':source['configuration'],'historical_expectations':expectation,'assertion_scope':['configuration_deserializes','runtime_adapter_maps']+(['historical_observation_is_preserved'] if obs else []),'limitations':['not_a_raw_http_response','not_evidence_of_current_live_output','not_independently_reviewed','tests_not_run_by_policy']}
  path=OUT/f'{sid}.json'; path.write_text(json.dumps(payload,ensure_ascii=False,indent=2)+'\n'); names.append(str(path.relative_to(ROOT)))
 index={'schema_version':1,'source_count':len(names),'contracts':names,'provenance_policy':'Contracts preserve configuration and historical observations. They do not synthesize response bodies or claim current extraction success.'}
 (OUT.parent/'contract-index.json').write_text(json.dumps(index,ensure_ascii=False,indent=2)+'\n')
 print(f'wrote {len(names)} evidence contracts')
if __name__=='__main__': main()
