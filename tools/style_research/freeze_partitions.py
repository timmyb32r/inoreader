"""Freeze metadata-only source-group partitions before author-body analysis.

Derived title/URL keys are only grouping features; original source records remain
unchanged. Near-duplicate source discoveries must be quarantined manually later.
"""
from collections import defaultdict, Counter
from datetime import datetime, timezone
import json
from pathlib import Path
import random
import re
import unicodedata
from urllib.parse import urlsplit, urlunsplit, parse_qsl, urlencode

STATE = Path('.inoreader-state/style-research')
SEED = 20260926
SEEN = set(range(1523, 1543))


def title_key(value):
    return ' '.join(re.findall(r'\w+', unicodedata.normalize('NFKC', value).casefold()))


def source_key(value):
    part = urlsplit(value)
    # Explicit grouping-only removal of analytics parameters, never a fetch rewrite.
    query = [(k, v) for k, v in parse_qsl(part.query, keep_blank_values=True) if not k.startswith('utm_')]
    return urlunsplit((part.scheme.casefold(), part.netloc.casefold(), part.path, urlencode(query), ''))


def freeze(index):
    by_id = {p['id']: p for p in index}
    parent = {p['id']: p['id'] for p in index}
    def root(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    keys = {}
    for post in index:
        values = [('title', title_key(t['text'])) for t in post['title_candidates'] if len(t['text']) >= 12]
        values += [('url', source_key(link['url'])) for link in post['links'] if link['external']]
        for key in values:
            if key in keys: parent[root(post['id'])] = root(keys[key])
            else: keys[key] = post['id']
    groups = defaultdict(list)
    for post in index: groups[root(post['id'])].append(post['id'])
    groups = {min(ids): sorted(ids) for ids in groups.values()}
    strata = defaultdict(list)
    for gid, ids in sorted(groups.items()):
        if SEEN.intersection(ids): continue
        candidates = []
        for i in ids:
            p = by_id[i]
            title = p['title_candidates'][0]['text'] if p['title_candidates'] else ''
            has_english_title = len(re.findall('[A-Za-z]', title)) >= 10 and len(re.findall('[А-Яа-я]', title)) < len(re.findall('[A-Za-z]', title))
            if not p['forwarded'] and p['timestamp'] and 12 <= len(title) <= 240 and p['text_length_chars'] >= 250 and (has_english_title or any(l['external'] for l in p['links'])):
                candidates.append(p)
        if candidates:
            p = candidates[0]
            stratum = p['timestamp'][:7] + ('/short' if p['text_length_chars'] < 1400 else '/long')
            strata[stratum].append(gid)
    randomizer = random.Random(SEED)
    for ids in strata.values(): randomizer.shuffle(ids)
    # Fixed rotating stratum order, 14 final + 6 validation groups per stratum.
    order = []
    for j in range(max(map(len, strata.values()), default=0)):
        for stratum in sorted(strata):
            if j < len(strata[stratum]): order.append((stratum, j, strata[stratum][j]))
    selection = {'final': [], 'validation': []}
    assigned = {gid: 'development' for gid in groups}
    for stratum, position, gid in order:
        split = 'final' if position < 14 else 'validation' if position < 20 else None
        if split:
            assigned[gid] = split
            selection[split].append({'group_id': gid, 'post_ids': groups[gid], 'stratum': stratum})
    protected_ids = {i for gid, ids in groups.items() if assigned[gid] != 'development' for i in ids}
    # Exclude adjacent possible continuations from tuning, without claiming identity.
    for gid, ids in groups.items():
        if assigned[gid] == 'development' and not SEEN.intersection(ids) and any(i - 1 in protected_ids or i + 1 in protected_ids for i in ids):
            assigned[gid] = 'quarantine_neighbor'
    return {
        'schema_version': 1, 'seed': SEED, 'created_at': datetime.now(timezone.utc).isoformat(),
        'already_seen_ids': sorted(SEEN), 'input_post_count': len(index),
        'selection_order': selection,
        'groups': [{'group_id': gid, 'post_ids': ids, 'split': assigned[gid]} for gid, ids in sorted(groups.items())],
        'limitations': ['Automatic title/URL keys cannot prove all semantic near-duplicates. Merge newly discovered source groups toward protected splits, quarantine any source seen in tuning.', 'Candidate eligibility is not a confirmed authorship/source-pair classification.', 'No final/validation post bodies have been read during freezing.']}


def main():
    out = STATE / 'partitions.json'
    if out.exists(): raise SystemExit('Partition already frozen; refusing overwrite')
    inventory = json.loads((STATE / 'corpus/inventory.json').read_text())
    if inventory['stop_reason'] != 'reached_message_id_1': raise SystemExit('Full traversal is not complete')
    index = [json.loads(line) for line in (STATE / 'corpus/index.jsonl').read_text().splitlines()]
    partition = freeze(index)
    out.write_text(json.dumps(partition, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'groups': dict(Counter(g['split'] for g in partition['groups'])), 'posts': dict(Counter(g['split'] for g in partition['groups'] for _ in g['post_ids']))}))

if __name__ == '__main__': main()
