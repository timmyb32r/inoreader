#!/usr/bin/env python3
"""Extract row-aware, review-only evidence from the raster Inoreader PDF.

Rows are discovered from the repeated blue enabled-toggle geometry, not from OCR
text. This keeps rows whose URL is unreadable. Tesseract output is attached to
each detected cell verbatim as tokens/lines and is never normalized into a URL.
The result is evidence for visual review, not a trusted import boundary.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from PIL import Image


EXPECTED_ROWS = 214
RENDER_DPI = 300
URLISH = re.compile(r"https?\s*[:/]", re.IGNORECASE)
MARKER_X = (900, 990)
CELL_X = (1080, 1570)


@dataclass(frozen=True)
class Marker:
    page: int
    box: tuple[int, int, int, int]
    image_width: int
    image_height: int

    @property
    def center_y(self) -> int:
        return self.box[1] + self.box[3] // 2


def tesseract_tsv(image: Path) -> tuple[list[dict[str, object]], list[str]]:
    completed = subprocess.run(
        ["tesseract", str(image), "stdout", "--psm", "6", "tsv"],
        check=True,
        text=True,
        capture_output=True,
    )
    words: list[dict[str, object]] = []
    grouped: dict[tuple[int, int, int], list[str]] = {}
    for word in csv.DictReader(io.StringIO(completed.stdout), delimiter="\t"):
        text = word.get("text", "")
        if word.get("level") != "5" or not text.strip():
            continue
        token = {
            "text": text,
            "confidence": float(word["conf"]),
            "box": [int(word["left"]), int(word["top"]), int(word["width"]), int(word["height"])],
            "line": [int(word["block_num"]), int(word["par_num"]), int(word["line_num"])],
        }
        words.append(token)
        grouped.setdefault(tuple(token["line"]), []).append(text)
    lines = [" ".join(tokens) for tokens in grouped.values()]
    return words, lines


def is_marker_pixel(rgb: tuple[int, int, int]) -> bool:
    red, green, blue = rgb
    return blue > 140 and blue - red > 40 and blue - green > 5 and max(rgb) - min(rgb) > 45


def connected_components(image: Image.Image) -> list[tuple[int, int, int, int, int]]:
    pixels = image.load()
    points = {
        (x, y)
        for y in range(image.height)
        for x in range(MARKER_X[0], MARKER_X[1])
        if is_marker_pixel(pixels[x, y])
    }
    components = []
    while points:
        seed = points.pop()
        stack = [seed]
        component = [seed]
        while stack:
            x, y = stack.pop()
            for neighbour in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if neighbour in points:
                    points.remove(neighbour)
                    stack.append(neighbour)
                    component.append(neighbour)
        xs = [point[0] for point in component]
        ys = [point[1] for point in component]
        components.append(
            (min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1, len(component))
        )
    return components


def markers_for_page(image: Image.Image, page: int) -> list[Marker]:
    markers = []
    for left, top, width, height, area in connected_components(image):
        # The toggle has an invariant 61x38 shape. A page boundary may clip its
        # height, so height is deliberately allowed down to 15 pixels.
        if 908 <= left <= 918 and 58 <= width <= 64 and height >= 15 and area > 500:
            markers.append(Marker(page, (left, top, width, height), image.width, image.height))
    return sorted(markers, key=lambda marker: marker.center_y)


def marker_fragments(markers_by_page: list[list[Marker]]) -> list[list[Marker]]:
    fragments = [[marker] for page in markers_by_page for marker in page]
    merged: list[list[Marker]] = []
    index = 0
    while index < len(fragments):
        current = fragments[index]
        if index + 1 < len(fragments):
            marker = current[0]
            following = fragments[index + 1][0]
            if (
                marker.page + 1 == following.page
                and marker.box[3] < 30
                and following.box[3] < 30
                and marker.box[3] + following.box[3] >= 36
            ):
                merged.append([marker, following])
                index += 2
                continue
        merged.append(current)
        index += 1
    return merged


def vertical_bounds(page_markers: list[Marker], index: int) -> tuple[int, int]:
    marker = page_markers[index]
    center = marker.center_y
    if index == 0:
        distance = page_markers[1].center_y - center if len(page_markers) > 1 else 112
        top = max(0, center - distance // 2)
    else:
        top = (page_markers[index - 1].center_y + center) // 2
    if index + 1 == len(page_markers):
        distance = center - page_markers[index - 1].center_y if index else 112
        bottom = min(marker.image_height, center + distance // 2)
    else:
        bottom = (center + page_markers[index + 1].center_y) // 2
    # A marker clipped by a page boundary identifies a row spanning two rendered
    # pages. Preserve the whole visible fragment instead of discarding the strip
    # between the midpoint estimate and the physical page edge.
    if marker.box[3] < 30 and index == 0:
        top = 0
    if marker.box[3] < 30 and index + 1 == len(page_markers):
        bottom = marker.image_height
    return top, bottom


def confidence(words: list[dict[str, object]]) -> float | None:
    values = [float(word["confidence"]) for word in words if float(word["confidence"]) >= 0]
    return round(sum(values) / len(values), 2) if values else None


def extract_fragment(
    image: Image.Image,
    marker: Marker,
    page_markers: list[Marker],
    scratch: Path,
) -> dict[str, object]:
    marker_index = page_markers.index(marker)
    top, bottom = vertical_bounds(page_markers, marker_index)
    crop_box = (CELL_X[0], top, CELL_X[1], bottom)
    crop_path = scratch / f"page-{marker.page:02d}-row-{marker_index + 1:02d}.png"
    image.crop(crop_box).save(crop_path)
    words, lines = tesseract_tsv(crop_path)
    return {
        "page": marker.page,
        "page_image_size": [marker.image_width, marker.image_height],
        "marker_box": list(marker.box),
        "cell_crop_box": [crop_box[0], crop_box[1], crop_box[2] - crop_box[0], crop_box[3] - crop_box[1]],
        "raw_ocr_lines": lines,
        "raw_ocr_words": words,
        "ocr_confidence": confidence(words),
    }


def row_from_fragments(row_number: int, fragments: list[dict[str, object]]) -> dict[str, object]:
    lines = [line for fragment in fragments for line in fragment["raw_ocr_lines"]]
    url_index = next((index for index, line in enumerate(lines) if URLISH.search(line)), None)
    name_lines = lines[:url_index] if url_index is not None else lines
    url_lines = lines[url_index:] if url_index is not None else []
    confidences = [fragment["ocr_confidence"] for fragment in fragments if fragment["ocr_confidence"] is not None]
    uncertainty = ["unreviewed_ocr"]
    if len(fragments) > 1:
        uncertainty.append("row_split_across_pdf_pages")
    if not url_lines:
        uncertainty.append("url_not_recognized")
    if len(url_lines) > 1:
        uncertainty.append("url_or_cell_wrapped_across_ocr_lines")
    if confidences and min(confidences) < 70:
        uncertainty.append("low_ocr_confidence")
    return {
        "row_number": row_number,
        "name_ocr_lines": name_lines,
        "url_ocr_lines": url_lines,
        "ocr_confidence": round(sum(confidences) / len(confidences), 2) if confidences else None,
        "uncertainty": uncertainty,
        "review_status": "needs_visual_review",
        "fragments": fragments,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("pdf", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.pdf.is_file():
        raise SystemExit(f"PDF does not exist: {args.pdf}")
    with tempfile.TemporaryDirectory(prefix="inoreader-pdf-") as directory:
        scratch = Path(directory)
        prefix = scratch / "page"
        subprocess.run(
            ["pdftoppm", "-jpeg", "-r", str(RENDER_DPI), str(args.pdf), str(prefix)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        image_paths = sorted(prefix.parent.glob("page-*.jpg"), key=lambda path: int(path.stem.split("-")[-1]))
        images = [Image.open(path).convert("RGB") for path in image_paths]
        markers_by_page = [markers_for_page(image, page) for page, image in enumerate(images, 1)]
        logical_markers = marker_fragments(markers_by_page)
        rows = []
        for row_number, markers in enumerate(logical_markers, 1):
            fragments = [
                extract_fragment(images[marker.page - 1], marker, markers_by_page[marker.page - 1], scratch)
                for marker in markers
            ]
            rows.append(row_from_fragments(row_number, fragments))

    detected_fragments = sum(len(page) for page in markers_by_page)
    split_rows = sum(1 for row in rows if len(row["fragments"]) > 1)
    status = "complete_geometry_needs_visual_review" if len(rows) == EXPECTED_ROWS else "row_count_mismatch"
    payload = {
        "schema_version": 2,
        "source_pdf": args.pdf.name,
        "render_dpi": RENDER_DPI,
        "expected_rows_from_ui": EXPECTED_ROWS,
        "detected_marker_fragments": detected_fragments,
        "split_rows_merged": split_rows,
        "detected_logical_rows": len(rows),
        "page_row_fragments": [len(page) for page in markers_by_page],
        "status": status,
        "note": "Geometry accounts for every UI row. OCR remains untrusted; exact names and URLs require visual review. URL OCR is preserved as lines and is never silently joined or repaired.",
        "rows": rows,
    }
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {len(rows)} unreviewed logical rows from {detected_fragments} marker fragments "
        f"({split_rows} split row) to {args.output}"
    )
    if len(rows) != EXPECTED_ROWS:
        raise SystemExit(f"detected {len(rows)} rows; expected {EXPECTED_ROWS}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
