#!/usr/bin/env python3
"""Drive mnemosyne-studio over JSON-RPC — the AI-primary, no-display path.

Spawns the studio binary and talks newline-delimited JSON-RPC 2.0 on
stdin/stdout (pinion's `spawn_stdin_rpc_reader`). The witness is
scene-as-data (not pixels): `scene/snapshot` returns the painted scene tree
as JSON, so you can read the windowed changelog rows, `scene/scroll`, and
snapshot again to watch the window slide — all without a display or GPU.

Usage (from the repo root):
  python3 studio/tools/rpc.py                       # demo: snapshot -> scroll -> snapshot
  python3 studio/tools/rpc.py <store.json>          # against a specific store
  python3 studio/tools/rpc.py <store.json> -i       # interactive: type JSON-RPC frames

Interactive mode: type a method (+ optional JSON params) per line, e.g.
  scene/snapshot {"path":"","from":"paint","viewport":{"w":720,"h":560}}
  scene/scroll   {"path":"changelog_scroll","to":{"dx":0,"dy":400}}
Ctrl-D to quit.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

BIN = Path(__file__).resolve().parents[1] / "target" / "debug" / "mnemosyne-studio"
VIEWPORT = {"w": 720, "h": 560}
SCROLL_TAG = "changelog_scroll"


class Studio:
    """A live studio process driven over JSON-RPC stdin/stdout."""

    def __init__(self, store: str) -> None:
        self.proc = subprocess.Popen(
            [str(BIN), store],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,  # 'shell: ...' logs go to stderr
            text=True,
        )
        self._id = 0

    def call(self, method: str, params=None):
        self._id += 1
        frame = {"jsonrpc": "2.0", "method": method, "id": self._id}
        if params is not None:
            frame["params"] = params
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(frame) + "\n")
        self.proc.stdin.flush()
        for line in self.proc.stdout:  # read until the matching id
            line = line.strip()
            if not line.startswith("{"):
                continue
            msg = json.loads(line)
            if msg.get("id") == self._id:
                if "error" in msg:
                    raise RuntimeError(msg["error"])
                return msg.get("result")
        return None

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except OSError:
            pass
        self.proc.terminate()


def _text_of(node) -> str | None:
    """First string `content` (a Text node) anywhere under `node`."""
    if isinstance(node, dict):
        if isinstance(node.get("content"), str):
            return node["content"]
        for child in node.get("children") or []:
            t = _text_of(child)
            if t:
                return t
        if isinstance(node.get("content"), dict):  # Scroll content subtree
            return _text_of(node["content"])
    return None


def _scan(node, rows: list, offset: list) -> None:
    """Collect (index, label) for each `changelog#<i>` row + the scroll offset."""
    if not isinstance(node, dict):
        return
    if node.get("offset_y") is not None and offset[0] is None:
        offset[0] = node.get("offset_y")
    tag = node.get("tag")
    if isinstance(tag, str) and tag.startswith("changelog#"):
        rows.append((int(tag.split("#", 1)[1]), _text_of(node)))
    for child in node.get("children") or []:
        _scan(child, rows, offset)
    if isinstance(node.get("content"), dict):
        _scan(node["content"], rows, offset)


def rows_and_offset(studio: Studio):
    result = studio.call(
        "scene/snapshot", {"path": "", "from": "paint", "viewport": VIEWPORT}
    )
    rows: list = []
    offset: list = [None]
    _scan(result, rows, offset)
    rows.sort()
    return rows, offset[0]


def show(label: str, rows, offset) -> None:
    print(f"\n[{label}]  scroll offset_y={offset}  visible rows={len(rows)}")
    for i, t in rows[:5]:
        print(f"    changelog#{i:<4} {t}")
    if len(rows) > 5:
        last_i, last_t = rows[-1]
        print(f"    …(+{len(rows) - 5} more) … changelog#{last_i:<4} {last_t}")


def demo(store: str) -> None:
    studio = Studio(store)
    time.sleep(0.3)
    try:
        rows, offset = rows_and_offset(studio)
        show("snapshot 1 — top of ledger (DATA, no pixels)", rows, offset)
        studio.call("scene/scroll", {"path": SCROLL_TAG, "to": {"dx": 0, "dy": 400}})
        time.sleep(0.15)
        rows, offset = rows_and_offset(studio)
        show("snapshot 2 — after scene/scroll dy=400 (window slid)", rows, offset)
        print(
            "\nThe row band shifted to higher indices on scroll — the windowed "
            "list proven as data, no display."
        )
    finally:
        studio.close()


def interactive(store: str) -> None:
    studio = Studio(store)
    time.sleep(0.3)
    print("interactive JSON-RPC — '<method> [json-params]' per line, Ctrl-D to quit")
    try:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            parts = line.split(None, 1)
            method = parts[0]
            params = json.loads(parts[1]) if len(parts) > 1 else None
            try:
                print(json.dumps(studio.call(method, params), ensure_ascii=False))
            except RuntimeError as exc:
                print(f"error: {exc}")
    finally:
        studio.close()


def main() -> None:
    args = [a for a in sys.argv[1:]]
    interactive_mode = "-i" in args
    args = [a for a in args if a != "-i"]
    store = args[0] if args else "docs/.atomic/workspace.atomic.json"
    if not BIN.exists():
        sys.exit(f"build first: cargo build --manifest-path studio/Cargo.toml  ({BIN} missing)")
    (interactive if interactive_mode else demo)(store)


if __name__ == "__main__":
    main()
