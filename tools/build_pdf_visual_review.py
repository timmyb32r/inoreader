#!/usr/bin/env python3
"""Build a self-contained visual-review form from the PDF and OCR candidates."""
import argparse,base64,html,io,json,subprocess,tempfile
from pathlib import Path
from PIL import Image

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('pdf',type=Path); ap.add_argument('candidates',type=Path); ap.add_argument('output',type=Path); ap.add_argument('--inventory',type=Path); args=ap.parse_args()
 data=json.loads(args.candidates.read_text())
 if data['status']!='complete_geometry_needs_visual_review': raise SystemExit('candidate geometry is incomplete')
 reviewed={}
 if args.inventory:
  inventory=json.loads(args.inventory.read_text(encoding='utf-8'))
  reviewed={row['row_number']:row for row in inventory['pdf_inventory']['rows']}
 with tempfile.TemporaryDirectory(prefix='inoreader-review-') as td:
  prefix=Path(td)/'page'; subprocess.run(['pdftoppm','-jpeg','-r',str(data['render_dpi']),str(args.pdf),str(prefix)],check=True)
  pages={int(p.stem.split('-')[-1]):Image.open(p).convert('RGB') for p in Path(td).glob('page-*.jpg')}
  cards=[]
  for row in data['rows']:
   images=[]
   for frag in row['fragments']:
    x,y,w,h=frag['cell_crop_box']; crop=pages[frag['page']].crop((x,y,x+w,y+h)); buf=io.BytesIO(); crop.save(buf,'JPEG',quality=88)
    images.append(f'<img alt="row {row["row_number"]} page {frag["page"]}" src="data:image/jpeg;base64,{base64.b64encode(buf.getvalue()).decode()}">')
   accepted=reviewed.get(row['row_number']); name=accepted['name'] if accepted else ' '.join(row['name_ocr_lines']); url=accepted['url'] if accepted else ' '.join(row['url_ocr_lines']); checked=' checked' if accepted else ''
   cards.append(f'''<section data-row="{row['row_number']}"><h2>Row {row['row_number']}</h2>{''.join(images)}<label>Name <input class="name" value="{html.escape(name,quote=True)}"></label><label>URL <input class="url" value="{html.escape(url,quote=True)}"></label><label><input class="reviewed" type="checkbox"{checked}> Exact values visually reviewed</label><pre>{html.escape(', '.join(row['uncertainty']))}</pre></section>''')
 script='''function save(){const rows=[...document.querySelectorAll('section')].map(s=>({row_number:+s.dataset.row,name:s.querySelector('.name').value,url:s.querySelector('.url').value,review_status:s.querySelector('.reviewed').checked?'reviewed':'needs_visual_review'}));const reviewed=rows.filter(r=>r.review_status==='reviewed');const blob=new Blob([JSON.stringify({schema_version:1,source_pdf:''' + json.dumps(args.pdf.name) + ''',reviewed_rows:reviewed},null,2)+'\\n'],{type:'application/json'});const a=document.createElement('a');a.href=URL.createObjectURL(blob);a.download='pdf-reviewed-rows.json';a.click()}'''
 page='''<!doctype html><meta charset="utf-8"><title>Inoreader PDF visual review</title><style>body{font:16px system-ui;max-width:1050px;margin:auto}header{position:sticky;top:0;background:white;padding:12px;border-bottom:2px solid #167d78;z-index:2}section{border:1px solid #94a3b8;padding:12px;margin:16px 0}img{max-width:100%;display:block;margin:8px 0}label{display:block;margin:8px 0}input.name,input.url{width:95%;padding:7px}pre{white-space:pre-wrap;color:#475569}</style><header><b>214-row visual review</b> — compare each crop with the editable exact values, tick only after checking, then <button onclick="save()">Download reviewed JSON</button></header>''' + ''.join(cards)+f'<script>{script}</script>'
 args.output.write_text(page,encoding='utf-8'); print(f'wrote {len(cards)} review rows to {args.output}')
if __name__=='__main__': main()
