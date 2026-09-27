import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from collect_channel import collect, missing_ranges, parse_page, plain_text, write_artifacts


def fixture(post_id=12, forwarded=False, body=None):
    body = '<b>Original title</b><br/>A &amp; B<br><a href="https://example.com/article?a=1&amp;b=2">Source</a>' if body is None else body
    forward = '<div class="tgme_widget_message_forwarded_from"><a href="https://t.me/someone">Someone</a></div>' if forwarded else ''
    return f'''<!doctype html><div class="tgme_widget_message_wrap">
<div class="tgme_widget_message js-widget_message" data-post="reading_data_news/{post_id}">
{forward}<div class="tgme_widget_message_text js-message_text">{body}</div>
<a class="tgme_widget_message_link_preview" href="https://example.com/article"><div class="link_preview_title">Preview title</div><div>Preview body</div></a>
<a href="https://t.me/reading_data_news/{post_id}"><time datetime="2024-12-31T23:01:02+00:00">date</time></a>
</div></div>'''


class ParserTests(unittest.TestCase):
    def test_exact_html_and_explicit_provenance_are_preserved(self):
        html = fixture()
        provenance = {"requested_url": "https://t.me/s/reading_data_news", "fetched_at": "2026-09-26T21:00:00Z"}
        post, = parse_page(html, "reading_data_news", provenance)
        self.assertEqual(post["id"], 12)
        self.assertEqual(post["timestamp"], "2024-12-31T23:01:02+00:00")
        self.assertEqual(post["formatted_html"], '<b>Original title</b><br/>A &amp; B<br><a href="https://example.com/article?a=1&amp;b=2">Source</a>')
        self.assertEqual(post["text"], "Original title\nA & B\nSource")
        self.assertIn(post["raw_post_html"], html)
        self.assertEqual(post["links"][0]["url"], "https://example.com/article?a=1&b=2")
        self.assertEqual(len(post["links"]), 2)
        self.assertEqual(post["provenance"], [provenance])
        self.assertEqual(post["title_candidates"][0]["evidence"], "first_text_line_unverified_title")
        self.assertEqual(post["descriptive_type"], "nonforwarded_linked_text")

    def test_forward_is_retained_but_not_assumed_authored(self):
        post, = parse_page(fixture(forwarded=True), "reading_data_news", {})
        self.assertTrue(post["forwarded"])
        self.assertEqual(post["descriptive_type"], "forwarded")
        self.assertIn("Someone", post["forwarded_from_html"])

    def test_empty_and_unsupported_posts_not_dropped(self):
        html = '<div class="tgme_widget_message text_not_supported" data-post="reading_data_news/42"><a>Media not supported</a></div>'
        post, = parse_page(html, "reading_data_news", {})
        self.assertEqual(post["id"], 42)
        self.assertEqual(post["text"], "")
        self.assertEqual(post["descriptive_type"], "media_or_unavailable_text")
        self.assertIsNone(post["timestamp"])
        self.assertEqual(post["raw_post_html"], html)

    def test_multiple_posts_and_nested_same_tags_remain_separate(self):
        posts = parse_page(fixture(2) + fixture(7, body="<div>One<div>Two</div>Three</div>"), "reading_data_news", {})
        self.assertEqual([post["id"] for post in posts], [2, 7])
        self.assertEqual(posts[1]["formatted_html"], "<div>One<div>Two</div>Three</div>")
        self.assertNotIn("Original title", posts[1]["raw_post_html"])

    def test_wrong_channel_and_malformed_timestamp_fail_explicitly(self):
        with self.assertRaises(ValueError):
            parse_page(fixture().replace('data-post="reading_data_news/', 'data-post="someone/'), "reading_data_news", {})
        with self.assertRaises(ValueError):
            parse_page(fixture().replace("2024-12-31T23:01:02+00:00", "not-a-date"), "reading_data_news", {})

    def test_decode_once_not_recursively(self):
        self.assertEqual(plain_text("&amp;lt;strong&amp;gt;<br> <code>raw &lt;b&gt;</code>"), "&lt;strong&gt;\n raw <b>")

    def test_inventory_gaps_and_no_full_text_in_index(self):
        posts = parse_page(fixture(2) + fixture(5), "reading_data_news", {})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inventory = write_artifacts(root, {post["id"]: post for post in posts}, [], "test")
            self.assertEqual(inventory["missing_id_ranges_within_observed_span"], [[3, 4]])
            self.assertEqual(inventory["post_count"], 2)
            self.assertEqual(len((root / "posts.jsonl").read_text().splitlines()), 2)
            index = json.loads((root / "index.jsonl").read_text().splitlines()[0])
            self.assertNotIn("text", index)
            self.assertNotIn("formatted_html", index)
            self.assertNotIn("text", index["links"][0])
            self.assertIn("title_candidates", index)
        self.assertEqual(missing_ranges([1, 2, 4, 9]), [[3, 3], [5, 8]])

    def test_pagination_preserves_overlap_and_stops_when_no_older_ids_exist(self):
        bodies = [fixture(8) + fixture(9), fixture(4) + fixture(8), fixture(4)]

        def fake_fetch(url, html_path, headers_path, timeout, transport):
            html_path.write_text(bodies.pop(0))
            headers_path.write_text("HTTP/2 200\n")
            return url, 200

        with tempfile.TemporaryDirectory() as directory, patch("collect_channel.fetch", side_effect=fake_fetch):
            result = collect(Path(directory), 0, 1, False)
            self.assertEqual(result["post_count"], 3)
            self.assertEqual(result["stop_reason"], "public_preview_returned_no_older_ids")
            self.assertEqual(len(result["pages"]), 3)
            posts = [json.loads(line) for line in (Path(directory) / "posts.jsonl").read_text().splitlines()]
            self.assertEqual(len(posts[0]["provenance"]), 2)
            self.assertEqual(len(posts[1]["provenance"]), 2)
            with patch("collect_channel.fetch", side_effect=AssertionError("Must not refetch archived pages")):
                replay = collect(Path(directory), 0, 1, True)
            self.assertEqual(replay["post_count"], 3)

    def test_conflicting_snapshots_fail_without_overwriting_first_record(self):
        bodies = [fixture(8), fixture(4) + fixture(8, body="Changed version")]

        def fake_fetch(url, html_path, headers_path, timeout, transport):
            html_path.write_text(bodies.pop(0))
            return url, 200

        with tempfile.TemporaryDirectory() as directory, patch("collect_channel.fetch", side_effect=fake_fetch):
            with self.assertRaisesRegex(ValueError, "conflicting post versions for ID 8"):
                collect(Path(directory), 0, 1, False)
            root = Path(directory)
            self.assertIn("Changed version", (root / "raw" / "before-00000008.html").read_text())
            records = [json.loads(line) for line in (root / "posts.jsonl").read_text().splitlines()]
            self.assertEqual(next(post for post in records if post["id"] == 8)["text"], "Original title\nA & B\nSource")


if __name__ == "__main__":
    unittest.main()
