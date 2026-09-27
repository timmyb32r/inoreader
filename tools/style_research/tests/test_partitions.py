import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('partition', Path(__file__).parents[1] / 'freeze_partitions.py')
partition = importlib.util.module_from_spec(spec)
spec.loader.exec_module(partition)


def post(i, title, url=None):
    return {'id': i, 'title_candidates': [{'text': title}], 'links': [{'external': True, 'url': url}] if url else [], 'forwarded': False, 'timestamp': '2026-08-01T00:00:00+00:00', 'text_length_chars': 700}

class PartitionTests(unittest.TestCase):
    def test_same_title_and_source_are_never_split(self):
        rows = [post(1, 'Example of a long article', 'https://example.com/a?utm_source=r'), post(2, 'Example of a long article'), post(3, 'Other name for same source', 'https://example.com/a')]
        groups = partition.freeze(rows)['groups']
        self.assertEqual(len(groups), 1)
        self.assertEqual(groups[0]['post_ids'], [1,2,3])

    def test_seen_sample_and_duplicate_are_development(self):
        groups = partition.freeze([post(1523, 'Example of a long article'), post(50, 'Example of a long article')])['groups']
        self.assertEqual(groups[0]['split'], 'development')

    def test_seed_reproduces_selections_and_input_is_unchanged(self):
        import copy
        rows = [post(i, f'Article about databases number {i}') for i in range(1,100)]
        unchanged = copy.deepcopy(rows)
        self.assertEqual(partition.freeze(rows)['selection_order'], partition.freeze(rows)['selection_order'])
        self.assertEqual(rows, unchanged)
        self.assertTrue(any(g['split'] == 'quarantine_neighbor' for g in partition.freeze(rows)['groups']))

if __name__ == '__main__': unittest.main()
