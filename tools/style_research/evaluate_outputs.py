"""Deterministic transport/quotation checks and descriptive signals, not an LLM judge."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re


def article_snapshot(request):
    """Read a recorded draft/review request without changing its source text.

    Historical research review framing is accepted explicitly, not inferred
    from arbitrary JSON. Unknown framing and trailing data fail closed.
    """
    content = request['messages'][1]['content']
    prefix, body = content.split('\n', 1)
    payload, end = json.JSONDecoder().raw_decode(body)
    suffix = body[end:]
    if prefix == 'ARTICLE_SNAPSHOT (untrusted source data):':
        if suffix != "\n\nSummarize this article in the requested author's style. Preserve the original title exactly.":
            raise ValueError('Unexpected draft request suffix')
        snapshot = payload
    elif prefix in (
        'ARTICLE_SNAPSHOT and DRAFT_SUMMARY (untrusted source data):',
        'ARTICLE_SNAPSHOT and DRAFT_SUMMARY (untrusted data):',
    ):
        allowed_suffix = '\n\nVerify and correct this draft against the complete article. Preserve its style.'
        if suffix not in ('', allowed_suffix) or set(payload) != {'article_snapshot', 'draft_summary'}:
            raise ValueError('Unexpected review request envelope')
        snapshot = payload['article_snapshot']
    else:
        raise ValueError('Unknown article request framing')
    if not isinstance(snapshot, dict) or set(snapshot) != {'title', 'url', 'text'} or any(
        not isinstance(value, str) or not value for value in snapshot.values()
    ):
        raise ValueError('Invalid complete article snapshot')
    return snapshot


def evaluate(response, snapshot):
    choice = response['choices'][0]
    flags = []
    if choice['finish_reason'] != 'stop': flags.append('incomplete_response_' + str(choice['finish_reason']))
    try: payload = json.loads(choice['message']['content'])
    except (ValueError, TypeError): return {'flags': flags + ['invalid_json'], 'complete': False}
    if type(payload) is not dict or set(payload) != {'segments'} or type(payload['segments']) is not list or not payload['segments']:
        return {'flags': flags + ['invalid_envelope'], 'complete': False}
    rendered = []
    for segment in payload['segments']:
        if type(segment) is not dict or set(segment) != {'kind','content'} or segment['kind'] not in ('text','quote') or not isinstance(segment['content'], str) or not segment['content']:
            flags.append('invalid_segment');continue
        text = segment['content']
        if segment['kind'] == 'quote':
            if text not in snapshot['text']: flags.append('unverified_quote')
            rendered.append('\n'.join('> ' + l for l in text.split('\n')))
        else:
            if re.search(r'^\s*>', text, re.M): flags.append('blockquote_in_unverified_text')
            rendered.append(text)
    content = '\n\n'.join(rendered)
    first_line = content.split('\n', 1)[0]
    title_without_markup = re.sub(r'^#{1,6} ', '', first_line)
    if title_without_markup.startswith('**') and title_without_markup.endswith('**'):
        title_without_markup = title_without_markup[2:-2]
    if title_without_markup != snapshot['title']:
        flags.append('title_changed_or_missing')
    elif first_line != '**' + snapshot['title'] + '**':
        flags.append('title_format_not_bold')
    body = content.split('\n',1)[1] if '\n' in content else ''
    numbers = lambda value: set(re.findall(r'(?<![\w])\d+(?:[.,]\d+)*(?![\w])',value))
    return {'flags': flags, 'complete': not flags, 'rendered': content,
        'chars': len(content), 'body_words': len(body.split()), 'paragraphs': len(rendered),
        'bold_fragments': len(re.findall(r'\*\*.+?\*\*',content)),
        'numeric_tokens_not_literal_in_source': sorted(numbers(content) - numbers(snapshot['text'] + snapshot['title'])),
        'note': 'Numeric flags require manual semantic review; no automatic factual-truth claim'}


def draft_can_be_reviewed(evaluation):
    """Match production: title/style defects do not bypass the reviewer.

    Only a complete, parseable, quotation-valid draft may reach the second
    stage. Exact-title/style flags remain recorded and the final acceptance
    check still requires all flags to be absent.
    """
    repairable = {'title_changed_or_missing', 'title_format_not_bold'}
    return 'rendered' in evaluation and set(evaluation['flags']) <= repairable


def main():
    parser=argparse.ArgumentParser();parser.add_argument('directory',type=Path);args=parser.parse_args()
    rows=[]
    for response_path in args.directory.glob('*/response.json'):
        request=json.loads((response_path.parent/'request.json').read_text())
        snapshot=article_snapshot(request)
        result=evaluate(json.loads(response_path.read_text()),snapshot)
        (response_path.parent/'evaluation.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
        if 'rendered' in result:(response_path.parent/'rendered.md').write_text(result['rendered'])
        rows.append({'id':response_path.parent.name,**{k:v for k,v in result.items() if k not in ('rendered','note')}})
    print(json.dumps(rows,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
