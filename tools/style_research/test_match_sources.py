import json
from pathlib import Path
import tempfile
import unittest

from match_sources import assemble, match_title, match_explicit_url


class CohortTests(unittest.TestCase):
    def test_explicit_url_is_separate_evidence_and_cannot_hide_title_conflict(self):
        url = "https://publisher.example/paper.pdf"
        post = {"id": 9, "title_candidates": [{"text": url}], "links": [{"url": url, "external": True}]}
        source = {"title": "Exact paper heading", "requested_url": url}
        self.assertEqual(match_explicit_url(post, source)["kind"], "exact_explicit_primary_url_without_author_title")
        with self.assertRaisesRegex(ValueError, "No exact explicit"):
            match_explicit_url(post, {**source, "requested_url": url + "?different"})
        with self.assertRaisesRegex(ValueError, "URL-only"):
            match_explicit_url({**post, "title_candidates": [{"text": "Other article"}]}, source)
        with self.assertRaisesRegex(ValueError, "no exact title match"):
            match_title(post, source)

    def test_exact_title_evidence_preserves_original_values_and_rejects_fuzzy(self):
        post = {"id": 7, "title_candidates": [{"text": "Exact title", "evidence": "first_text_line_unverified_title"}]}
        source = {"title": "Exact title ", "requested_url": "https://publisher.example/article"}
        matched = match_title(post, source)
        self.assertEqual(matched["source_title_evidence"], "Exact title ")
        with self.assertRaisesRegex(ValueError, "no exact title match"):
            match_title(post, {**source, "title": "Similar exact title"})

    def test_unavailable_candidate_is_replaced_within_its_stratum_and_logged(self):
        selection = [
            {"group_id": 1, "post_ids": [1], "stratum": "July/short"},
            {"group_id": 2, "post_ids": [2], "stratum": "August/long"},
            {"group_id": 3, "post_ids": [3], "stratum": "July/short"},
        ]
        partitions = {"selection_order": {"final": selection}}
        index = {i: {"id": i, "title_candidates": [{"text": f"Title {i}", "evidence": "first_text_line_unverified_title"}]} for i in (1, 2, 3)}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            decisions = {"1": {"status": "unavailable", "reason": "HTTP403 challenge", "attempts": ["retained-raw-metadata.json"]}}
            for i in (2, 3):
                text_path = root / f"{i}.txt"
                text_path.write_text("Full original article\n")
                metadata = {"status": "extracted_unverified_pair", "title": f"Title {i}", "requested_url": f"https://publisher.example/{i}", "text_path": str(text_path), "raw_path": f"{i}.html", "text_chars": 21, "fetch_time": "2026-09-26T20:00:00Z", "transport": "direct", "extraction_selector": "//article"}
                path = root / f"{i}.json"
                path.write_text(json.dumps(metadata))
                decisions[str(i)] = {"status": "reviewed_source", "metadata_path": str(path), "boundary_review": "Full article inspected"}
            records = assemble(partitions, index, decisions, "final", 2)
            self.assertEqual([r["post_id"] for r in records], [1, 3, 2])
            self.assertEqual([r["selection_slot"] for r in records], [1, 1, 2])
            self.assertEqual(records[0]["status"], "unavailable")
            self.assertFalse(records[1]["content_alignment_verified"])
            url = "https://publisher.example/3"
            index[3] = {"id": 3, "title_candidates": [{"text": url}], "links": [{"url": url, "external": True}]}
            decisions["3"]["evidence_mode"] = "explicit_url_only"
            explicit = assemble(partitions, index, decisions, "final", 2)
            self.assertEqual(explicit[1]["status"], "confirmed_explicit_url")
            self.assertEqual(explicit[1]["title"], "Title 3")
            self.assertFalse(explicit[1]["content_alignment_verified"])
            decisions["3"]["evidence_mode"] = "guess"
            with self.assertRaisesRegex(ValueError, "Unknown source evidence"):
                assemble(partitions, index, decisions, "final", 2)
            with self.assertRaisesRegex(ValueError, "No reviewed decision"):
                assemble(partitions, index, {"1": decisions["1"]}, "final", 2)
            with self.assertRaisesRegex(ValueError, "exceeds"):
                assemble(partitions, index, decisions, "final", 9)

    def test_changed_source_text_cannot_be_silently_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'body.txt').write_text('silently shortened')
            metadata = {"status": "extracted_unverified_pair", "title": "Title", "requested_url": "https://publisher.example/", "text_path": str(root / 'body.txt'), "text_chars": 999}
            (root / 'source.json').write_text(json.dumps(metadata))
            partitions = {"selection_order": {"final": [{"group_id": 1, "post_ids": [1], "stratum": "A"}]}}
            index = {1: {"id": 1, "title_candidates": [{"text": "Title"}]}}
            with self.assertRaisesRegex(ValueError, "text length differs"):
                assemble(partitions, index, {"1": {"status": "reviewed_source", "metadata_path": str(root / 'source.json')}}, "final", 1)


if __name__ == '__main__':
    unittest.main()
