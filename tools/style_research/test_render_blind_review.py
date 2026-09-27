import copy
from html.parser import HTMLParser
import json
from pathlib import Path
import tempfile
import unittest

from render_blind_review import render_html, validate_manifest, write_review


def manifest(count=20):
    return {
        "schema_version": 1,
        "evaluation_id": "9e04bafa-43db-4373-8c19-bab8ef5f4a99",
        "items": [{"id": f"a8e1-{number:04d}", "title": f"Original title {number}",
                   "url": f"https://publisher.example/article/{number}",
                   "text": f"**Original title {number}**\n\nПолный пересказ {number}."}
                  for number in range(count)],
    }


class PageData(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.tags = []
        self.embedded = ""
        self.active = False
        self.feed(source)

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        self.tags.append((tag, attrs))
        self.active = tag == "script" and attrs.get("id") == "review-data"

    def handle_endtag(self, tag):
        if tag == "script":
            self.active = False

    def handle_data(self, data):
        if self.active:
            self.embedded += data


class BlindReviewTests(unittest.TestCase):
    def test_default_design_requires_twenty_without_dropping_or_filling_items(self):
        self.assertEqual(validate_manifest(manifest()), manifest())
        for count in (0, 19, 21):
            with self.assertRaisesRegex(ValueError, "exactly 20"):
                render_html(manifest(count))
        self.assertEqual(json.loads(PageData(render_html(manifest(2), 2)).embedded), manifest(2))
        for invalid in (True, 0, -1, 1.2):
            with self.assertRaises(ValueError):
                validate_manifest(manifest(), invalid)

    def test_rejects_labels_and_references_instead_of_embedding_hidden_identity(self):
        for field in ("model", "arm", "reference", "post_id", "label"):
            value = manifest()
            value[field] = "must not leak"
            with self.assertRaisesRegex(ValueError, "exactly these fields"):
                render_html(value)
            value = manifest()
            value["items"][0][field] = "must not leak"
            with self.assertRaisesRegex(ValueError, "exactly these fields"):
                render_html(value)

    def test_full_untrusted_text_round_trips_without_html_execution_or_template_expansion(self):
        value = manifest()
        value["items"][0]["text"] = (
            '</script><script>window.evil=true</script><img src="https://bad.example/pixel">'
            '\n@@STYLE@@ @@SCRIPT@@ @@NONCE@@ & < > \u2028 \u2029\r\n'
            + "Данные **не обрезать**\n" * 20000 + "LAST_SENTINEL"
        )
        value["items"][0]["title"] = '<svg onload="evil()">Exact & title</svg>'
        page = PageData(render_html(value))
        self.assertEqual(json.loads(page.embedded), value)
        self.assertEqual(len([tag for tag, _ in page.tags if tag == "script"]), 2)
        self.assertFalse(any(tag in ("img", "svg", "iframe", "object") for tag, _ in page.tags))
        self.assertTrue(json.loads(page.embedded)["items"][0]["text"].endswith("LAST_SENTINEL"))

    def test_invalid_identity_data_and_urls_fail_before_output(self):
        for field, invalid in (("id", "two words"), ("title", " "), ("text", ""), ("text", "\ud800")):
            value = manifest()
            value["items"][0][field] = invalid
            with self.assertRaises(ValueError):
                render_html(value)
        value = manifest(); value["items"][1]["id"] = value["items"][0]["id"]
        with self.assertRaisesRegex(ValueError, "duplicates"):
            render_html(value)
        for url in ("javascript:alert(1)", "data:text/html,evil", "//publisher.example/a", "/a", "file:///etc/passwd",
                    "https://user:password@publisher.example/", "https://publisher.example/a\nb", "https:\\publisher.example",
                    "https://publisher.example:99999/a", "http://[invalid]/", "https://"):
            value = manifest(); value["items"][0]["url"] = url
            with self.subTest(url=url), self.assertRaises(ValueError):
                render_html(value)
        value = manifest(); value["schema_version"] = True
        with self.assertRaises(ValueError):
            render_html(value)

    def test_exact_urls_and_whitespace_are_preserved(self):
        value = manifest()
        value["items"][0].update(url="https://publisher.example/a?x=1&y=2#section", title="  Exact title  ", text="  full\n\ttext\n")
        self.assertEqual(json.loads(PageData(render_html(value)).embedded), value)

    def test_csp_is_self_contained_and_nonce_matches_scripts_and_style(self):
        page = PageData(render_html(manifest()))
        policy = next(attrs["content"] for tag, attrs in page.tags if tag == "meta" and attrs.get("http-equiv") == "Content-Security-Policy")
        nonces = {attrs["nonce"] for tag, attrs in page.tags if tag in ("script", "style")}
        self.assertEqual(len(nonces), 1)
        self.assertIn(f"'nonce-{next(iter(nonces))}'", policy)
        for directive in ("default-src 'none'", "connect-src 'none'", "img-src 'none'", "base-uri 'none'"):
            self.assertIn(directive, policy)
        self.assertFalse(any(attrs.get("src") for tag, attrs in page.tags if tag == "script"))

    def test_file_creation_does_not_overwrite_existing_review(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.json"; output = root / "review/index.html"
            source.write_text(json.dumps(manifest()), encoding="utf-8")
            self.assertEqual(write_review(source, output), output)
            original = output.read_bytes()
            with self.assertRaises(FileExistsError):
                write_review(source, output)
            self.assertEqual(output.read_bytes(), original)
            invalid = copy.deepcopy(manifest()); invalid["items"] = []
            source.write_text(json.dumps(invalid))
            with self.assertRaises(ValueError):
                write_review(source, root / "must-not-exist.html")
            self.assertFalse((root / "must-not-exist.html").exists())


if __name__ == "__main__":
    unittest.main()
