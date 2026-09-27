#!/usr/bin/env python3
"""Reproducible SQL comparison; disposable pinned PostgreSQL, synthetic records only."""
import json,pathlib,re,subprocess,time,uuid,statistics
ROOT=pathlib.Path(__file__).resolve().parents[1]
IMAGE='postgres:17-bookworm@sha256:91eb910c44c7ed13f7f1a4ccadaa9ca72ef14cddc04cacb6e070e48eb44731a3'
name='reader-query-bench-'+uuid.uuid4().hex

def sql(text):
 return subprocess.check_output(['docker','exec','-i',name,'psql','-h','127.0.0.1','-X','-qAt','-v','ON_ERROR_STOP=1','-U','postgres'],input=text,text=True)

def explain_once(query):
 result=json.loads(sql('EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) '+query))[0]
 return {'execution_ms':result['Execution Time'],'planning_ms':result['Planning Time'],'plan':result['Plan']}

def explain(query):
 explain_once(query)
 samples=[explain_once(query) for _ in range(3)]
 selected=sorted(samples,key=lambda s:s['execution_ms'])[1]
 return {**selected,'samples_ms':[v['execution_ms'] for v in samples],'sql':query}

try:
 subprocess.run(['docker','run','-d','--name',name,'--tmpfs','/var/lib/postgresql/data:rw,size=8g','-e','POSTGRES_PASSWORD=benchmark-only',IMAGE],check=True,stdout=subprocess.DEVNULL)
 for _ in range(120):
  if subprocess.run(['docker','exec',name,'pg_isready','-h','127.0.0.1','-U','postgres'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0:break
  time.sleep(.5)
 else:raise RuntimeError('PostgreSQL did not start')
 schema=re.search(r'pub const SCHEMA_SQL: &str = r#"(.*?)"#;', (ROOT/'crates/reader-storage-postgres/src/schema.rs').read_text(),re.S)[1]
 sql(schema)
 sql("CREATE INDEX articles_old_comparison ON articles ((split_part(id, '/', 1)), ((document::jsonb ->> 'first_arrived_at')) DESC, id DESC);")
 workspace='00000000-0000-0000-0000-000000000001';report=[];previous=0
 for size in [10000,100000,1000000]:
  sql(f"""INSERT INTO articles(id,revision,document)
   SELECT '{workspace}/'||lpad(to_hex(i),32,'0')::uuid,0,
   json_build_object('id',lpad(to_hex(i),32,'0')::uuid,'first_arrived_at','2026-09-27T12:00:00.'||lpad((i%1000000000)::text,9,'0')||'Z','state',json_build_object('read',i%3=0,'later',i%11=0),'key',json_build_object('title',repeat('Synthetic article ',16),'description',repeat('Synthetic text ',64)))::text
   FROM generate_series({previous+1},{size}) i; VACUUM ANALYZE articles;""")
  old=f"SELECT document FROM articles WHERE split_part(id,'/',1)='{workspace}' AND NOT (document::jsonb#>>'{{state,read}}')::boolean ORDER BY (document::jsonb->>'first_arrived_at') DESC,id DESC LIMIT 51"
  new=f"SELECT document FROM articles WHERE workspace_key='{workspace}' AND NOT is_read ORDER BY arrival_order DESC,id DESC LIMIT 51"
  old_count=f"SELECT count(*) FROM articles WHERE split_part(id,'/',1)='{workspace}' AND NOT (document::jsonb#>>'{{state,read}}')::boolean"
  new_count=f"SELECT count(*) FROM articles WHERE workspace_key='{workspace}' AND NOT is_read"
  report.append({'rows':size,'old_page':explain(old),'projected_page':explain(new),'old_count':explain(old_count),'projected_count':explain(new_count)})
  previous=size
 output=ROOT/'docs/architecture/query-benchmark-2026-09-27.json';output.parent.mkdir(exist_ok=True);output.write_text(json.dumps({'image':IMAGE,'cache':'one warmup, median of three samples','both_old_and_new_indexes_present':True,'measurements':report},indent=2)+'\n')
 print(json.dumps([{k:v if k=='rows' else v['execution_ms'] for k,v in row.items()} for row in report],indent=2))
finally:
 subprocess.run(['docker','rm','-fv',name],stdout=subprocess.DEVNULL,check=False)
