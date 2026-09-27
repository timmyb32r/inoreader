# Glossary panel overflow and missing paragraph translation

The glossary's implicit CSS grid column expanded to the min-content width of a
long article title. Focusing the close button could horizontally scroll the
clipped panel. The column is now constrained to `minmax(0, 1fr)`, grid items may
shrink, and icon controls retain their dimensions. The toolbar label is `Terms`.

DeepSeek's cached TapData result contained the exact Chinese source in its
translation field. The prompt now prioritizes the complete Russian translation
before its independent word dictionary. Construction and deserialization reject
Chinese-source results without Russian letters. This is a necessary language
check, not a semantic quality guarantee; it neither rewrites source text nor
introduces automatic paid retries.

The single affected production job is marked failed, retaining its original
result and usage in the stored document and a private diagnostic backup. An
explicit replacement request checks the same article and preserves the source
and segmentation. No article or source record is removed.

Verification: the new long-title browser regression fails with the old CSS and
passes with the fix, including immediate pending feedback, deduplication and
stable close/copy targets. Rust tests reject echoed Chinese and English-only
responses through both provider parsing and deserialization. `just
check-affected` and the complete `just check-release` passed, including real
PostgreSQL, Chromium, frontend unit and browser tests.

Production verification: the same TapData paragraph completed on `deepseek-flash`
in 5.55 seconds, with 57 word annotations and unchanged source reconstruction.
The Russian translation was present, the second request reused its job, and the
temporary authenticated test session was removed. Estimated request cost:
$0.002033400. Deployed as `inoreader-app:reader-panels-20260927`.
