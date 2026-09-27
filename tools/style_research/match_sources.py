#!/usr/bin/env python3
"""Assemble frozen evaluation cohorts from explicitly reviewed source snapshots.

Reads metadata only, never Telegram post bodies. Selection follows frozen
round-robin stratum slots; an unavailable candidate is replaced by the next
candidate in the same stratum. All unsuccessful candidates remain in output.
"""

from collections import defaultdict, deque
import argparse
import json
from pathlib import Path


def match_title(post, source):
    """Exact title evidence after boundary-whitespace removal, no fuzzy matching."""
    original_titles = source.get("h1_titles", []) + source.get("metadata_titles", []) + [source["title"]]
    for candidate in post["title_candidates"]:
        if candidate["text"].startswith(("http://", "https://")):
            continue
        for original in original_titles:
            if candidate["text"].strip() == original.strip():
                return {
                    "kind": "exact_title_at_original_publisher",
                    "post_title_metadata": candidate,
                    "source_title_evidence": original,
                    "comparison": "exact Unicode after boundary whitespace trim; no case folding or fuzzy match",
                    "source_url": source["requested_url"],
                }
    raise ValueError(f"Post {post['id']} has no exact title match in the source snapshot")


def match_explicit_url(post, source):
    """Match URL-only post metadata without pretending an author title exists.

    This is an explicit evidence mode for PDF links. A conflicting non-URL title
    remains ambiguous and must not be bypassed through this mode. URLs are
    compared exactly, without normalization or canonical-URL guessing.
    """
    if any(not candidate["text"].startswith(("https://", "http://"))
           for candidate in post["title_candidates"]):
        raise ValueError("Explicit-URL evidence requires URL-only title metadata")
    matches = [link for link in post.get("links", [])
               if link.get("external") and link["url"] == source["requested_url"]]
    if not matches:
        raise ValueError("No exact explicit source URL in post metadata")
    return {"kind": "exact_explicit_primary_url_without_author_title",
            "post_link_metadata": matches[0], "source_url": source["requested_url"],
            "source_title_evidence": source["title"],
            "comparison": "Exact explicit hyperlink; author title unavailable; semantic alignment still unverified"}


def assemble(partitions, index, decisions, cohort, count):
    selection = partitions["selection_order"][cohort]
    if count < 1 or count > len(selection):
        raise ValueError("Requested cohort size exceeds frozen selection slots or is not positive")
    queues = defaultdict(deque)
    for group in selection:
        queues[group["stratum"]].append(group)
    records = []
    for slot, original in enumerate(selection[:count], start=1):
        stratum = original["stratum"]
        while queues[stratum]:
            group = queues[stratum].popleft()
            post = index[group["post_ids"][0]]
            decision = decisions.get(str(post["id"]))
            base = {"post_id": post["id"], "group_id": group["group_id"], "stratum": stratum, "selection_slot": slot, "cohort": cohort}
            if decision is None:
                raise ValueError(f"No reviewed decision for {cohort} candidate {post['id']}; do not silently skip")
            if decision["status"] == "unavailable":
                records.append({**base, "status": "unavailable", "reason": decision["reason"], "attempts": decision["attempts"]})
                continue
            if decision["status"] != "reviewed_source":
                raise ValueError("Unknown source decision")
            source = json.loads(Path(decision["metadata_path"]).read_text())
            if source["status"] != "extracted_unverified_pair":
                raise ValueError("Reviewed source extraction did not succeed")
            evidence_mode = decision.get("evidence_mode", "exact_title")
            if evidence_mode == "exact_title":
                evidence = match_title(post, source)
            elif evidence_mode == "explicit_url_only":
                evidence = match_explicit_url(post, source)
            else:
                raise ValueError("Unknown source evidence mode")
            text = Path(source["text_path"]).read_text()
            if len(text.rstrip("\n")) != source["text_chars"]:
                raise ValueError("Persisted source text length differs from extraction manifest")
            records.append({
                **base, "status": "confirmed_title_url" if evidence_mode == "exact_title" else "confirmed_explicit_url",
                "title": source["title"].strip(), "url": source["requested_url"],
                "title_evidence": {"raw_path": source.get("title_evidence_raw_path", source["raw_path"]), "selector": source.get("title_selector", "source snapshot original h1"), "exact_source_heading": source["title"]},
                "text_path": source["text_path"], "raw_path": source["raw_path"],
                "source_metadata_path": decision["metadata_path"], "fetch_time": source["fetch_time"],
                "extraction_selector": source["extraction_selector"], "text_chars": source["text_chars"],
                "transport": source["transport"], "input_kind": source.get("input_kind", "published_article_or_presentation_transcript"),
                "match_evidence": evidence, "boundary_review": decision["boundary_review"],
                "content_alignment_verified": False,
                "limitation": "Source identity evidence recorded explicitly; author-summary factual alignment must be evaluated after prompt freeze",
            })
            break
        else:
            raise ValueError(f"Exhausted stratum {stratum} before filling slot {slot}")
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(".inoreader-state/style-research"))
    parser.add_argument("--decisions", type=Path, required=True)
    parser.add_argument("--cohort", choices=("final", "validation"), required=True)
    parser.add_argument("--count", type=int, required=True)
    args = parser.parse_args()
    if args.count < 1:
        parser.error("count must be positive")
    partitions = json.loads((args.root / "partitions.json").read_text())
    index = {post["id"]: post for post in map(json.loads, (args.root / "corpus/index.jsonl").read_text().splitlines())}
    decisions = json.loads(args.decisions.read_text())[args.cohort]
    records = assemble(partitions, index, decisions, args.cohort, args.count)
    output = args.root / "pairs" / f"{args.cohort}.jsonl"
    output.parent.mkdir(exist_ok=True)
    if output.exists():
        raise ValueError("Pair manifest exists; preserve frozen cohort rather than overwrite")
    output.write_text("".join(json.dumps(record, ensure_ascii=False) + "\n" for record in records))
    ready_statuses = {"confirmed_title_url", "confirmed_explicit_url"}
    print(json.dumps({"cohort": args.cohort, "ready": [row["post_id"] for row in records if row["status"] in ready_statuses], "unavailable": [row["post_id"] for row in records if row["status"] == "unavailable"], "path": str(output)}))


if __name__ == "__main__":
    main()
