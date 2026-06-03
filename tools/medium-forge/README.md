# medium-forge

A **general-purpose** converter: structured HTML → a standard, epubcheck-clean
EPUB 3 + a neutral section-anchor map.

```
source HTML  +  anchor map { id → anchor }   ──▶   { spec.epub  +  anchors.json }
```

It knows nothing about any particular spec, naming scheme, or downstream
consumer. You supply the HTML and a map of *your* stable ids to the in-HTML
anchors they point at; medium-forge preserves the document's content verbatim,
sanitises it to valid XHTML5, labels each anchored element with your id, and
packages a universal EPUB plus a JSON map of `id → EPUB locator`.

> **Location note:** this lives in `mnemosyne/tools/` for now (a sibling utility,
> not part of the Mnemosyne core crates). It is deliberately Mnemosyne-agnostic
> so it can be extracted to its own repository once it grows. Mnemosyne is the
> current consumer (it reads `anchors.json` to bind facts to spec locations), but
> nothing here depends on Mnemosyne.

## What it does

1. **Locate** — for each `id → anchor` in the map, find the anchored element and
   ascend to its nearest section container (`<section>` or a div whose class is
   in `--section-classes`, default `div1..div4`; falls back to the element).
2. **Label** — set `id` = your stable id on that container (original id kept as
   `data-orig-id` for provenance).
3. **Publish** — emit the subtrees matched by `--content-xpath` (default
   `//body`), in document order, as one XHTML doc — content preserved verbatim
   (tables, code, nesting).
4. **Sanitise** — drop `<script>/<link>/<style>`, external images, `on*`
   handlers, obsolete/presentational attributes (`summary`, `border`, `name`, …);
   insert a `<dt>` before any dt-less `<dl>` (XHTML5 requires it).
5. **Emit** — `spec.epub` (valid EPUB 3) + `anchors.json` (`epub-anchor-map/v1`).

## Usage

```bash
python3 convert.py \
    --html        in.html \
    --anchor-map  map.json \
    --out         out/ \
    [--content-xpath   '//div[@class="div1"]'] \  # what to publish (default //body)
    [--section-classes 'div1,div2,div3,div4']  \  # section containers
    [--title T] [--revision R] [--source-url U]

epubcheck out/spec.epub      # → 0 errors / 0 warnings
```

`map.json` is `{ id: anchor }` where anchor may be a bare fragment `"Foo"`, a
`"...#Foo"` URL, or `{ "anchor": "Foo" }` / `{ "anchor_url": "...#Foo" }`.

### Worked example — W3C SCXML

The `id → anchor` map for the W3C SCXML Recommendation is produced by SCE's
extractor (`scxml-core-engine/.../scxml_toc_to_manifest.py`, which encodes the
W3C-specific section-id naming). medium-forge stays generic:

```bash
SNAP=…/spec-snapshot/scxml-REC-20150901.html
python3 …/scxml_toc_to_manifest.py --html "$SNAP" --anchor-map out/scxml-anchor-map.json
python3 convert.py --html "$SNAP" --anchor-map out/scxml-anchor-map.json --out out \
    --content-xpath "//div[@class='div1']" \
    --revision REC-scxml-20150901 --source-url https://www.w3.org/TR/scxml/ --title "W3C SCXML"
# → 196/196 ids, epubcheck 0 errors, 21 tables + 65 code blocks preserved
```

## Output

| File | What |
|---|---|
| `out/spec.epub` | standard EPUB 3 (epubcheck-clean) — a universal artifact |
| `out/anchors.json` | `epub-anchor-map/v1` — `id ↔ EPUB locator` (href + fragment + CFI) |

`anchors.json` shape:
```jsonc
{ "schema": "epub-anchor-map/v1",
  "epub": { "path": "spec.epub", "revision": "…", "source": { "kind": "html", "url": "…" } },
  "anchors": [ { "id": "…", "locator": { "spine_href": "OEBPS/spec.xhtml",
                 "fragment": "…", "cfi": "epubcfi(…)" },
                "confidence": 1.0, "needs_review": false } ] }
```

## Roadmap

- **html backend** — done (this).
- **pdf backend** — structure/table/figure reconstruction (hard); build when a
  PDF source actually needs converting. Assisted extraction (emit `confidence` /
  `needs_review` per id → human side-by-side verify).
- **scan backend** — OCR (hardest).
- **Per-chapter spine** + proper nav tree (v1 is a single `spec.xhtml`, so all
  internal `#anchor` links resolve).
- **Sub-element / figure-level CFI** — section-level is the baseline; the emitted
  CFI is a structural placeholder, viewers resolve by `fragment`.
- **Extract to own repo** once it grows beyond a single backend.

## Dependencies

`python3` + `lxml`. `epubcheck` to validate. No EPUB library (manual assembly).
