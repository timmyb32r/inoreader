#!/usr/bin/env python3
"""Streaming latency histogram from JSON application logs; never emit payloads.

Pass --budgets to enforce p95 upper-bound budgets in milliseconds. Histogram
buckets are powers of two microseconds, not operational rejection limits.
"""
import argparse
from collections import defaultdict
from decimal import Decimal
import json
import math
import re

UNITS = {"ns": Decimal(".001"), "µs": Decimal(1), "μs": Decimal(1), "us": Decimal(1), "ms": Decimal(1000), "s": Decimal(1000000)}

def observation(event):
    message, target = event.get("message", ""), event.get("target", "")
    if target == "reader_server::api_request":
        match = re.search(r"operation=(\S+).*elapsed_ms=(\d+)", message)
        if match: return "api:" + match[1], Decimal(match[2]) * 1000
    if target == "sqlx::query":
        match = re.search(r"elapsed=(\d+(?:\.\d+)?)(ns|µs|μs|us|ms|s)\b", message)
        if match: return "postgres:statement", Decimal(match[1]) * UNITS[match[2]]
    if target == "sqlx::pool::acquire":
        # SQLx 0.8 names this field 'aquired' (sic).
        match = re.search(r"aquired_after_secs=([\d.eE+-]+)", message)
        if match: return "postgres:acquire", Decimal(match[1]) * 1000000
    match = re.search(r"stage=(\w+).*elapsed_us=(\d+)", message)
    if match: return "stage:" + match[1], Decimal(match[2])
    match = re.search(r"(?:class=(\w+).*|ai_queue .*?)queue_wait_us=(\d+)", message)
    if match: return "queue:" + (match[1] or "translation"), Decimal(match[2])
    return None

def report(events):
    histograms = defaultdict(lambda: defaultdict(int))
    for event in events:
        sample = observation(event)
        if sample is None: continue
        key, us = sample
        if not us.is_finite() or us < 0: raise ValueError("invalid latency observation")
        ceiling = int(us.to_integral_value(rounding="ROUND_CEILING"))
        bucket = 0 if ceiling == 0 else 1 << (ceiling - 1).bit_length()
        histograms[key][bucket] += 1
    result = {}
    for key, bins in sorted(histograms.items()):
        count = sum(bins.values())
        def quantile(percent):
            threshold = math.ceil(count * percent / 100)
            seen = 0
            for upper, n in sorted(bins.items()):
                seen += n
                if seen >= threshold: return upper / 1000
        result[key] = {"count": count, "p50_upper_ms": quantile(50), "p95_upper_ms": quantile(95), "p99_upper_ms": quantile(99), "buckets_upper_us": dict(sorted(bins.items()))}
    return result

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log")
    parser.add_argument("--budgets", help="JSON object: metric name to p95 upper-bound ms")
    args = parser.parse_args()
    with open(args.log) as source: result = report(json.loads(line) for line in source if line.strip())
    print(json.dumps(result, indent=2))
    if args.budgets:
        with open(args.budgets) as source: budgets = json.load(source)
        failures = [key for key, maximum in budgets.items() if key not in result or result[key]["p95_upper_ms"] > maximum]
        if failures: raise SystemExit("Missing or over-budget metrics: " + ", ".join(failures))
