#!/usr/bin/env python3
"""medium-forge — general HTML -> EPUB converter with a section-anchor map.

Converts a structured HTML document into a standard, epubcheck-clean EPUB 3 while
preserving its content verbatim, and emits an anchor map linking caller-supplied
stable ids to locations inside the EPUB.

GENERAL-PURPOSE: knows nothing about any particular spec, naming scheme, or
downstream consumer. You supply:
  * the source HTML
  * an anchor map  { id -> anchor }  (the in-HTML anchor/id each stable id points
    at).  Value may be a bare fragment ("Foo"), a "...#Foo" URL, or
    { "anchor": "Foo" } / { "anchor_url": "...#Foo" }.
and choose, if the defaults don't fit your HTML:
  * --content-xpath     the region to publish              (default: //body)
  * --section-classes   div classes treated as a section   (default: div1..div4)
                        container (<section> always counts)

Output:
  <out>/spec.epub       standard EPUB 3 (epubcheck-clean)
  <out>/anchors.json    epub-anchor-map/v2  ( per id: EPUB locator (href+fragment
                        +cfi) + verbatim section text + its sha256 )

The EPUB is a universal artifact (any reader). The anchor map is a thin, neutral
JSON contract any system can consume.

Usage:
  convert.py --html in.html --anchor-map map.json --out out/ \\
             [--content-xpath '//div[@class="body"]'] \\
             [--title T] [--revision R] [--source-url U]
"""
import argparse, hashlib, json, os, zipfile
from urllib.parse import urldefrag
from lxml import html as L, etree

XHTML = "http://www.w3.org/1999/xhtml"
DROP_TAGS = {"script", "link", "style", "meta", "noscript"}
# obsolete / presentational attributes XHTML5 (epubcheck) rejects
DROP_ATTRS = {"name", "border", "cellpadding", "cellspacing", "valign", "align",
              "bgcolor", "nowrap", "frame", "rules", "width", "height", "hspace",
              "vspace", "clear", "compact", "type", "start", "lang", "summary",
              "char", "charoff", "axis", "abbr", "scope", "headers"}


def anchor_of(value):
    """Accept a bare fragment, a '...#frag' URL, or a dict carrying either."""
    if isinstance(value, dict):
        value = value.get("anchor") or value.get("anchor_url") or ""
    return urldefrag(value)[1] or value


def section_container(el, section_classes):
    """Nearest enclosing section-like element (a <section> or a div with a
    configured section class); falls back to the element itself."""
    for anc in [el, *el.iterancestors()]:
        if not isinstance(anc.tag, str):
            continue
        if anc.tag == "section":
            return anc
        if anc.tag == "div" and (anc.get("class") or "") in section_classes:
            return anc
    return el


def locate(root, anchor_to_id, section_classes):
    out = {}
    for anchor, sid in anchor_to_id.items():
        node = root.xpath("(//*[@id=$a]|//*[@name=$a])[1]", a=anchor)
        if node:
            out[sid] = section_container(node[0], section_classes)
    return out


def sanitise(el):
    """Drop scripts/links/styles, external images, on* handlers, obsolete attrs;
    ensure every <dl> starts with <dt>. Structure/text/tables/code are kept."""
    for bad in el.xpath(".//*"):
        if bad.tag in DROP_TAGS and bad.getparent() is not None:
            bad.getparent().remove(bad)
    for node in [el, *el.iterdescendants()]:
        if not isinstance(node.tag, str):
            continue
        if node.tag == "img" and node.get("src", "").startswith("http"):
            if node.getparent() is not None:
                node.getparent().remove(node)
            continue
        for attr in list(node.attrib):
            if attr in DROP_ATTRS or attr.startswith("on"):
                del node.attrib[attr]
    for dl in el.xpath(".//dl"):
        first = next((c for c in dl if isinstance(c.tag, str)), None)
        if first is not None and first.tag == "dd":
            first.addprevious(etree.Element("dt"))


def to_xhtml_namespace(el):
    """Re-root an lxml.html element (no namespace) into the XHTML namespace."""
    raw = etree.tostring(el)
    tag = el.tag
    return etree.fromstring(
        raw.replace(b"<" + tag.encode(),
                    b'<%b xmlns="%b"' % (tag.encode(), XHTML.encode()), 1))


def build_xhtml(content_nodes, title):
    html_el = etree.Element(f"{{{XHTML}}}html", nsmap={None: XHTML})
    html_el.set("lang", "en")
    head = etree.SubElement(html_el, f"{{{XHTML}}}head")
    etree.SubElement(head, f"{{{XHTML}}}title").text = title
    etree.SubElement(head, f"{{{XHTML}}}style").text = (
        "body{font-family:Georgia,serif;line-height:1.5;max-width:46em;"
        "margin:2em auto;padding:0 1em}h1,h2,h3,h4{font-family:sans-serif}"
        "table{border-collapse:collapse;margin:1em 0}td,th{border:1px solid #ccc;"
        "padding:3px 8px}pre{background:#f4f1e9;padding:8px 12px;overflow:auto}")
    body = etree.SubElement(html_el, f"{{{XHTML}}}body")
    etree.SubElement(body, f"{{{XHTML}}}h1").text = title
    for node in content_nodes:
        # each matched node's whole subtree is published, in document order;
        # if the match is a wrapper <body>, publish its element children instead
        targets = [c for c in node if isinstance(c.tag, str)] if node.tag == "body" else [node]
        for t in targets:
            body.append(to_xhtml_namespace(t))
    return html_el


def write_epub(out_epub, spec_bytes, title, revision, source_url):
    ns = XHTML
    nav = (f'<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE html>\n'
           f'<html xmlns="{ns}" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">'
           f'<head><title>Contents</title></head><body>'
           f'<nav epub:type="toc" id="toc"><h1>Contents</h1>'
           f'<ol><li><a href="spec.xhtml">{title}</a></li></ol></nav></body></html>'
           ).encode()
    opf = (f'<?xml version="1.0" encoding="utf-8"?>\n'
           f'<package xmlns="http://www.idpf.org/2007/opf" version="3.0" '
           f'unique-identifier="bookid"><metadata '
           f'xmlns:dc="http://purl.org/dc/elements/1.1/">'
           f'<dc:identifier id="bookid">urn:uuid:5b3e8a2c-1f4d-4c9a-9e2b-7a6f0c1d2e3f</dc:identifier>'
           f'<dc:title>{title}</dc:title><dc:language>en</dc:language>'
           f'<dc:source>{source_url}</dc:source>'
           f'<meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>'
           f'</metadata><manifest>'
           f'<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>'
           f'<item id="spec" href="spec.xhtml" media-type="application/xhtml+xml"/>'
           f'</manifest><spine><itemref idref="spec"/></spine></package>').encode()
    container = ('<?xml version="1.0" encoding="utf-8"?>\n<container version="1.0" '
                 'xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>'
                 '<rootfile full-path="OEBPS/content.opf" '
                 'media-type="application/oebps-package+xml"/></rootfiles></container>'
                 ).encode()
    if os.path.exists(out_epub):
        os.remove(out_epub)
    with zipfile.ZipFile(out_epub, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip",
                    compress_type=zipfile.ZIP_STORED)
        zf.writestr("META-INF/container.xml", container)
        zf.writestr("OEBPS/content.opf", opf)
        zf.writestr("OEBPS/nav.xhtml", nav)
        zf.writestr("OEBPS/spec.xhtml", spec_bytes)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--html", required=True)
    ap.add_argument("--anchor-map", required=True)
    ap.add_argument("--out", default="out")
    ap.add_argument("--content-xpath", default="//body",
                    help="region to publish (default: //body)")
    ap.add_argument("--section-classes", default="div1,div2,div3,div4",
                    help="div classes treated as section containers")
    ap.add_argument("--title", default="Document (medium-forge EPUB)")
    ap.add_argument("--revision", default="")
    ap.add_argument("--source-url", default="")
    a = ap.parse_args()

    section_classes = set(filter(None, a.section_classes.split(",")))
    root = L.parse(a.html).getroot()
    amap = json.load(open(a.anchor_map))
    anchor_to_id = {anchor_of(v): sid for sid, v in amap.items()}

    located = locate(root, anchor_to_id, section_classes)
    for sid, el in located.items():
        if el.get("id"):
            el.set("data-orig-id", el.get("id"))
        el.set("id", sid)

    content_nodes = root.xpath(a.content_xpath)
    if not content_nodes:
        raise SystemExit(f"--content-xpath matched nothing: {a.content_xpath}")
    for n in content_nodes:
        sanitise(n)

    xhtml = build_xhtml(content_nodes, a.title)
    spec_bytes = (b'<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE html>\n'
                  + etree.tostring(xhtml, method="xml", encoding="utf-8"))

    os.makedirs(a.out, exist_ok=True)
    write_epub(os.path.join(a.out, "spec.epub"), spec_bytes, a.title,
               a.revision, a.source_url)

    anchors = []
    for sid in amap:
        if sid not in located:
            continue
        # Verbatim section text as published in the EPUB, whitespace-collapsed
        # for determinism. This is the normative excerpt the Rust store caches
        # (NormativeExcerpt.text); text_sha256 lets the store re-hash and detect
        # drift offline without re-extracting (epub-anchor-map/v2).
        text = " ".join(located[sid].text_content().split())
        anchors.append({
            "id": sid,
            "locator": {"spine_href": "OEBPS/spec.xhtml", "fragment": sid,
                        "cfi": f"epubcfi(/6/4!/4/*[@id='{sid}'])"},
            "text": text,
            "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
            "confidence": 1.0, "needs_review": False,
        })
    json.dump({"schema": "epub-anchor-map/v2",
               "epub": {"path": "spec.epub", "revision": a.revision,
                        "source": {"kind": "html", "url": a.source_url}},
               "anchors": anchors},
              open(os.path.join(a.out, "anchors.json"), "w"),
              ensure_ascii=False, indent=1)

    missing = [s for s in amap if s not in located]
    print(f"ids in map: {len(amap)} | located+emitted: {len(located)} | missing: {len(missing)}")
    if missing:
        print("  missing (first 10):", missing[:10])


if __name__ == "__main__":
    main()
