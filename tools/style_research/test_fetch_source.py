import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from fetch_source import content_text, derive_article, extract, fetch_article, validate_url


class SourceTests(unittest.TestCase):
    def test_semantic_body_preserves_every_paragraph_and_code(self):
        body = b'<html><head><meta property="og:title" content="Actual title"></head><body><header><h1>Website</h1></header><nav>Navigation</nav><article><p>First &amp; exact</p><pre>a &lt; b\nline 2</pre><p>Final paragraph</p><script>secretJs()</script></article><footer>Footer</footer></body></html>'
        result = extract(body, min_text_chars=1)
        self.assertEqual(result["title"], "Actual title")
        self.assertIn("First & exact", result["text"])
        self.assertIn("a < b\nline 2", result["text"])
        self.assertIn("Final paragraph", result["text"])
        self.assertNotIn("Navigation", result["text"])
        self.assertNotIn("Footer", result["text"])
        self.assertNotIn("secretJs", result["text"])

    def test_article_title_beats_site_name_and_ambiguous_body_requires_selector(self):
        body = b'<h1>Publication name</h1><article><h1>Article name</h1><p>The entire article.</p></article>'
        self.assertEqual(extract(body, min_text_chars=1)["title"], "Article name")
        with self.assertRaisesRegex(ValueError, "unambiguous"):
            extract(b'<h1>Site</h1><article>A</article><article>B</article>', min_text_chars=1)
        with self.assertRaisesRegex(ValueError, "matched 2"):
            extract(b'<h1>Site</h1><article>A</article><article>B</article>', "//article", 1)

    def test_rendered_single_article_heading_beats_og_brand_suffix(self):
        page = b'<meta property="og:title" content="Article: corrected | Publisher"><article><header><h1>Article: corrected</h1></header><div class="post-content">Complete body text.</div></article>'
        result = extract(page, min_text_chars=1)
        self.assertEqual(result['title'], 'Article: corrected')
        self.assertEqual(result['metadata_titles'], ['Article: corrected | Publisher'])
        self.assertIn('//h1', result['title_selector'])

    def test_large_article_is_not_truncated_and_configured_minimum_is_explicit(self):
        text = "Distinct paragraph " * 20000
        result = extract(f'<h1>Title</h1><article>{text}</article>'.encode())
        self.assertEqual(result["text"], text.strip())
        with self.assertRaisesRegex(ValueError, "configured minimum"):
            extract(b'<h1>Title</h1><article>Short</article>', min_text_chars=100)

    def test_userinfo_plaintext_and_private_ips_are_rejected(self):
        for url in ['http://example.com/x', 'https://user:secret@example.com/', 'https://127.0.0.1/', 'https://169.254.169.254/']:
            with self.assertRaises(ValueError):
                validate_url(url)
        validate_url('https://example.com/article')

    def test_immutable_fetch_and_local_derivative_keep_raw_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def run(command, **kwargs):
                Path(command[command.index('--output') + 1]).write_text('<h1>Title</h1><article><p>First original paragraph.</p><div id="detail">Detailed main content.</div></article>')
                return type('Result', (), {'returncode': 0, 'stdout': 'https://example.com/article\n200', 'stderr': ''})()

            with patch('fetch_source.subprocess.run', side_effect=run) as mocked:
                first = fetch_article('https://example.com/article', 'first', root, min_text_chars=1)
                same = fetch_article('https://example.com/article', 'first', root, min_text_chars=1)
                self.assertEqual(mocked.call_count, 1)
            self.assertEqual(first, same)
            before = Path(first['raw_path']).read_bytes()
            derived = derive_article(root / 'first.json', 'second', selector="//*[@id='detail']", min_text_chars=1)
            self.assertEqual(Path(derived['text_path']).read_text().strip(), 'Detailed main content.')
            self.assertEqual(Path(first['raw_path']).read_bytes(), before)
            with self.assertRaises(ValueError):
                fetch_article('https://different.example.com/', 'first', root)
            with self.assertRaises(ValueError):
                derive_article(root / 'first.json', 'second', selector="//*[@id='detail']")


if __name__ == '__main__':
    unittest.main()
