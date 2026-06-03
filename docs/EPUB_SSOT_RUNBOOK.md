# EPUB-as-content-SSOT Adoption Runbook

How an external-spec mirror workspace (e.g. an engine that cites a W3C / IETF /
IEEE / AUTOSAR standard) makes a committed **EPUB the single source of truth**
for its normative text, with offline revalidation. This is the consumer-facing
companion to `SCHEMA_GUIDE.md` (schema reference) and is the runbook the SCE
196-section W3C SCXML ledger follows.

## The model

- **EPUB** = baseline content SSOT for the spec text (renders in any reader;
  `medium-forge` produces a standard, fidelity-first EPUB from HTML).
- **Atomic store** = structure SSOT + facts + per-section pointers. Each
  mirrored section is an `AtomicSection` carrying a `normative_excerpt`:
  - `text` — a **derived cache** of the EPUB section's text (overwrite-allowed).
  - `text_sha256` — the offline revalidation anchor (sha256 of `text`).
  - `anchor_url` + `source_revision` — **authored identity** (which upstream
    section + revision this mirrors). These are *not* in the EPUB; you supply
    them from the spec's table of contents.
- **Provenance pins** (`[workspace.spec_source]`): `epub_path` + `epub_sha256`
  pin the committed EPUB file; `revision` is the rev the workspace tracks.

The Rust core **never re-extracts** from the EPUB — it only re-hashes the
cached string and the committed file. Extraction is the Python tool's job
(hexagonal boundary).

## One-time adoption

### 1. Produce the EPUB

```bash
# from your spec's HTML (see tools/medium-forge/README)
python3 tools/medium-forge/convert.py \
  --content-xpath "//div[@class='div1']" \
  --anchor-map toc-anchor-map.json \
  --revision "REC-scxml-20150901" \
  --source-url "https://www.w3.org/TR/scxml/" \
  --title "SCXML" --out out/
# → out/spec.epub  +  out/anchors.json  (epub-anchor-map/v2: per-section
#   text + text_sha256 + locator)
```

### 2. Author the section manifest

`anchors.json` carries `text` + `text_sha256` but **not** `anchor_url` /
`source_revision` (those are upstream identity, not EPUB content). Join the v2
map with your TOC to build an `import-sections` manifest:

```json
[
  { "section_id": "scxml-3.13", "parent_doc": "docs/spec.epub",
    "title": "Selecting Transitions",
    "normative_excerpt": {
      "text": "<text from anchors.json>",
      "text_sha256": "<text_sha256 from anchors.json>",
      "anchor_url": "https://www.w3.org/TR/scxml/#selecting-transitions",
      "source_revision": "REC-scxml-20150901" } }
]
```

`import-sections` routes every excerpt through one validator that verifies
`sha256(text) == text_sha256`, so a mis-joined manifest is rejected at import.

```bash
mnemosyne-cli import-sections --manifest manifest.json
```

### Choosing the text model

`text_sha256` anchors whichever text is **authoritative** — pick the source:

- **EPUB-projected** — the EPUB extraction *is* your text. Use
  `import-epub-excerpts` (above / below).
- **Consumer-authored** — your own extractor produces the text (e.g. at a
  granularity finer than the EPUB `div` scope) and *that* is the SSOT; the EPUB
  is pinned for provenance only. Author the text via `import-sections`, then
  **seal** it.

> **Do not blindly `import-epub-excerpts` over hand-authored excerpts.**
> `medium-forge` extracts the *container subtree* (`div1`..`div4`
> `text_content()`, including sub-sections and the heading), which can be far
> coarser than a heading→next-heading direct-body excerpt. Projecting it would
> overwrite precise excerpts with collapsed/overlapping blobs. If your extractor
> is the SSOT, **seal**, don't project.

**Migration A — EPUB is your text** (empty-hash excerpts, EPUB granularity OK):

```bash
mnemosyne-cli report-excerpt-hash-backfill          # lists empty-hash excerpts
mnemosyne-cli import-epub-excerpts --anchors out/anchors.json
mnemosyne-cli report-excerpt-hash-backfill          # now empty
```

**Migration B — your extractor is the SSOT** (text already in the store, just
seal it as its own baseline; `text` is never touched):

```bash
mnemosyne-cli report-excerpt-hash-backfill          # lists empty-hash excerpts
mnemosyne-cli seal-excerpt-hashes                   # text_sha256 = sha256(text)
mnemosyne-cli report-excerpt-hash-backfill          # now empty
mnemosyne-cli validate-content-drift                # sealed text revalidatable
```

### 3. Pin and commit the EPUB

Commit the revision-pinned EPUB under `docs/.atomic/epub/` and pin it in
`mnemosyne.toml`:

```toml
[workspace.spec_source]
url = "https://www.w3.org/TR/scxml/"
revision = "REC-scxml-20150901"
epub_path = "docs/.atomic/epub/scxml-REC-20150901.epub"
epub_sha256 = "<sha256 of the committed .epub>"   # 64-char lowercase hex

[content_drift]
severity = "reject"   # default; the gate for CI
```

`epub_path` + `epub_sha256` are a pair (both or neither).

### 4. Wire the CI gate

```bash
mnemosyne-cli validate-content-drift   # EPUB-file pinned + every cache matches its hash
mnemosyne-cli validate-spec-drift      # no Active section trails the workspace revision
mnemosyne-cli validate-code-refs       # citations resolve to live sections (if used)
```

A green `validate-content-drift` means: the committed EPUB equals the pinned
`epub_sha256` **and** every excerpt's cached `text` still hashes to its
`text_sha256`. Empty-hash (unrevalidatable) excerpts are counted but never gate
— resolve them with `import-epub-excerpts` (see `report-excerpt-hash-backfill`).

## Upstream revision change (the Layer B loop)

When the standard publishes a new revision:

1. Replace the committed EPUB (re-run `medium-forge` against the new HTML).
2. `validate-content-drift` now flags **EPUB-file drift** — the file no longer
   matches the pinned `epub_sha256`. This is the trigger to re-project.
3. `mnemosyne-cli import-epub-excerpts --anchors out/anchors.json` refreshes
   every cached `text` + `text_sha256` from the new EPUB.
4. For sections whose *meaning* changed across the revision, model the rev bump
   per `SCHEMA_GUIDE.md` (supersede the old section, create a new one) so the
   audit trail records which revision each excerpt mirrors.
5. Update `epub_sha256` (and `revision`) in `mnemosyne.toml` to the new file.
6. Re-run `validate-content-drift` → green.

The diff between old and new extraction (which sections actually changed) is a
CI step on the `medium-forge` output, not a Rust primitive — the core only
re-hashes.

## Caveats

- `anchor_url` is consumer-authored (from the TOC), not extracted from the
  EPUB — the join in step 2 is required for greenfield imports.
- `medium-forge` v1 emits a single-spine `spec.xhtml`; per-chapter spine split
  is a future enhancement (not required for the store/validation loop).
- This loop is fidelity-first and offline; it does not fetch the upstream at
  validation time (provenance is pinned, not live).
