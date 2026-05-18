---
namespace: journal
created: 2026-05-09T03:29:42Z
updated: 2026-05-09T03:29:42Z
status: resolved
tags: failure, fsv, jq, evidence-schema
---

# Failure: Readback Query Assumed Wrong Evidence Schema

## What Failed

During final physical readback, two `jq` summary commands failed with:

`Cannot iterate over null (null)`

## Root Cause

The source-of-truth JSON files existed and were valid, but my first readback selectors assumed normalized arrays
named `.cases` and `.happy_paths`. The current evidence files store mutation cases as named top-level objects
(`case_known_good`, `case_off_by_one`, and similar) and tier-compression happy paths under
`.happy_path_per_width`.

## Resolution

Inspected the actual source-of-truth keys with `jq 'keys'` and reran readbacks using the real schema. Corrected
queries confirmed the expected mutation cases, edge cases, split assignments, tier-compression blobs, and failure
codes were physically present.

## Lesson

For FSV reports, inspect the source-of-truth schema first, then summarize it. Do not treat a stale readback
selector as evidence.
