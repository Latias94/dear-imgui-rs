"""
API surface report for dear-imgui-rs.

This script compares the public ImGui API surface (as seen by cimgui generator
metadata) against the high-level crate implementation in `dear-imgui/src`.

It is meant to:
- identify candidate TODOs for high-level wrappers;
- reduce accidental duplicate wrappers by making coverage visible.

Notes:
- We classify `imgui_internal:*` entries as internal and exclude them from the
  public surface report by default.
- Coverage is considered present if either:
  - `dear-imgui/src` references the corresponding `sys::ig*` symbol, OR
  - `dear-imgui/src` contains `#[doc(alias = "...")]` for the ImGui function name.
  This intentionally over-approximates “coverage” to reduce false positives from
  wrapper indirection and alternate implementations.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Iterable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFS_JSON = REPO_ROOT / "dear-imgui-sys" / "third-party" / "cimgui" / "generator" / "output" / "definitions.json"
DEAR_IMGUI_SRC = REPO_ROOT / "dear-imgui" / "src"


@dataclass(frozen=True)
class PublicFunc:
    funcname: str
    ov_cimguiname: str
    location: str  # e.g. "imgui:533"


def _load_public_imgui_funcs(defs_path: pathlib.Path) -> list[PublicFunc]:
    obj = json.loads(defs_path.read_text(encoding="utf-8", errors="ignore"))
    out: list[PublicFunc] = []
    for overloads in obj.values():
        if not isinstance(overloads, list):
            continue
        for o in overloads:
            if o.get("namespace") != "ImGui":
                continue
            loc = str(o.get("location") or "")
            if loc.startswith("imgui_internal:"):
                continue
            func = o.get("funcname")
            ov = o.get("ov_cimguiname") or o.get("cimguiname")
            if not func or not ov:
                continue
            out.append(PublicFunc(funcname=str(func), ov_cimguiname=str(ov), location=loc))
    return out


def _iter_rs_files(root: pathlib.Path) -> Iterable[pathlib.Path]:
    yield from root.rglob("*.rs")


def _collect_sys_usages(rs_files: Iterable[pathlib.Path]) -> set[str]:
    used: set[str] = set()
    pat = re.compile(r"\b(?:crate::)?sys::(ig[A-Za-z0-9_]+)\b")
    for p in rs_files:
        t = p.read_text(encoding="utf-8", errors="ignore")
        used.update(pat.findall(t))
    return used


def _collect_doc_aliases(rs_files: Iterable[pathlib.Path]) -> set[str]:
    aliases: set[str] = set()
    pat = re.compile(r'alias\s*=\s*"([^"]+)"')
    for p in rs_files:
        t = p.read_text(encoding="utf-8", errors="ignore")
        aliases.update(pat.findall(t))
    return aliases


@dataclass(frozen=True)
class PublicFuncGroup:
    funcname: str
    ov_cimguinames: tuple[str, ...]
    location: str  # representative location


def _group_by_funcname(funcs: list[PublicFunc]) -> dict[str, PublicFuncGroup]:
    by: dict[str, list[PublicFunc]] = {}
    for f in funcs:
        by.setdefault(f.funcname, []).append(f)

    out: dict[str, PublicFuncGroup] = {}
    for funcname, items in by.items():
        items_sorted = sorted(items, key=lambda x: (x.ov_cimguiname, x.location))
        rep = items_sorted[0]
        syms = tuple(sorted({x.ov_cimguiname for x in items_sorted}))
        out[funcname] = PublicFuncGroup(funcname=funcname, ov_cimguinames=syms, location=rep.location)
    return dict(sorted(out.items(), key=lambda kv: kv[0]))


def _print_table(rows: list[tuple[str, str, str]]) -> None:
    # Minimal markdown table to paste into issues/PRs.
    print("| ImGui func | sys symbol | location |")
    print("|---|---|---|")
    for func, ov, loc in rows:
        print(f"| `{func}` | `{ov}` | `{loc}` |")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--format", choices=["md", "plain"], default="plain")
    ap.add_argument("--limit", type=int, default=200)
    args = ap.parse_args()

    public_funcs = _load_public_imgui_funcs(DEFS_JSON)
    groups = _group_by_funcname(public_funcs)

    rs_files = list(_iter_rs_files(DEAR_IMGUI_SRC))
    used_sys = _collect_sys_usages(rs_files)
    aliases = _collect_doc_aliases(rs_files)

    uncovered: list[PublicFuncGroup] = []
    for funcname, g in groups.items():
        if funcname in aliases:
            continue
        if set(g.ov_cimguinames) & used_sys:
            continue
        uncovered.append(g)

    uncovered = uncovered[: max(0, args.limit)]

    if args.format == "md":
        rows: list[tuple[str, str, str]] = []
        for g in uncovered:
            sys_sym = g.ov_cimguinames[0] if g.ov_cimguinames else "?"
            if len(g.ov_cimguinames) > 1:
                sys_sym = f"{sys_sym} (+{len(g.ov_cimguinames)-1})"
            rows.append((g.funcname, sys_sym, g.location))
        _print_table(rows)
        return 0

    covered_by_alias = len(set(groups) & aliases)
    covered_by_sys = 0
    for g in groups.values():
        if set(g.ov_cimguinames) & used_sys:
            covered_by_sys += 1

    print(f"Repo root: {REPO_ROOT}")
    print(f"Public ImGui funcnames: {len(groups)}")
    print(f"Covered via doc aliases: {covered_by_alias}")
    print(f"Covered via sys usage (any overload): {covered_by_sys}")
    print(f"Uncovered (representative): {len(uncovered)} (limited to {args.limit})")
    for g in uncovered:
        sys_sym = g.ov_cimguinames[0] if g.ov_cimguinames else "?"
        extra = ""
        if len(g.ov_cimguinames) > 1:
            extra = f" (+{len(g.ov_cimguinames)-1})"
        print(f"- {g.funcname}  sys={sys_sym}{extra}  loc={g.location}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
