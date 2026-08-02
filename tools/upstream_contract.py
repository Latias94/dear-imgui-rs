"""Audit maintained upstream API facts against explicit review decisions.

This module is intentionally a compact stable facade. Source adapters, schema,
and review validation live in dedicated modules so their failure modes can be
tested independently while existing maintenance scripts retain one entry point.
"""

from __future__ import annotations

import argparse
import pathlib
import shutil
import sys
from typing import Sequence


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
CI_TOOLS = REPO_ROOT / "tools" / "ci"
if str(CI_TOOLS) not in sys.path:
    sys.path.insert(0, str(CI_TOOLS))

INVENTORY_JSON = REPO_ROOT / "tools" / "build-support" / "maintained_sources.json"
BASELINE_JSON = REPO_ROOT / "tools" / "upstream_contract_baseline.json"
SNAPSHOT_JSON = REPO_ROOT / "tools" / "upstream_contract_snapshot.json"
DECISIONS_JSON = REPO_ROOT / "tools" / "upstream_contract_decisions.json"
PENDING_DECISIONS_JSON = REPO_ROOT / "tools" / "upstream_contract_decisions.pending.json"

from upstream_contract_model import (  # noqa: E402
    CLASSIFICATIONS,
    EVIDENCE_KINDS,
    FACT_KINDS,
    REVIEW_SCHEMA,
    RUNTIME_SHAPED_KINDS,
    SAFE_CLASSIFICATIONS,
    SHA1_RE,
    SNAPSHOT_SCHEMA,
    ApiDelta,
    ApiFact,
    ApiSnapshot,
    ContractAudit,
    ContractInputError,
    JsonReader,
    ReviewEvidence,
    ReviewGroup,
    ReviewManifest,
)
from upstream_contract_review import (  # noqa: E402
    audit_review_manifest as _audit_review_manifest,
    compare_snapshots,
    load_review_manifest,
    load_snapshot,
    review_template,
    write_snapshot,
)
from upstream_contract_sources import (  # noqa: E402
    collect_generator_facts,
    collect_rust_bindings_facts,
    collect_snapshot_at_source_revisions as _collect_snapshot_at_source_revisions,
    collect_repository_snapshot as _collect_repository_snapshot,
    resolve_nested_source_revisions as _resolve_nested_source_revisions,
)


def collect_repository_snapshot(
    repo_root: pathlib.Path = REPO_ROOT,
    inventory_path: pathlib.Path = INVENTORY_JSON,
) -> ApiSnapshot:
    return _collect_repository_snapshot(repo_root, inventory_path)


def collect_snapshot_at_source_revisions(
    source_revisions: dict[str, str],
    *,
    repo_root: pathlib.Path = REPO_ROOT,
    inventory_path: pathlib.Path = INVENTORY_JSON,
) -> ApiSnapshot:
    return _collect_snapshot_at_source_revisions(
        source_revisions, repo_root=repo_root, inventory_path=inventory_path
    )


def resolve_nested_source_revisions(
    top_level_revisions: dict[str, str],
    *,
    repo_root: pathlib.Path = REPO_ROOT,
    inventory_path: pathlib.Path = INVENTORY_JSON,
) -> dict[str, str]:
    return _resolve_nested_source_revisions(
        top_level_revisions, repo_root=repo_root, inventory_path=inventory_path
    )


def audit_review_manifest(
    baseline: ApiSnapshot,
    candidate: ApiSnapshot,
    manifest: ReviewManifest,
    *,
    repo_root: pathlib.Path = REPO_ROOT,
) -> ContractAudit:
    return _audit_review_manifest(baseline, candidate, manifest, repo_root=repo_root)


def audit_repository_contract(
    *,
    repo_root: pathlib.Path = REPO_ROOT,
    inventory_path: pathlib.Path = INVENTORY_JSON,
    baseline_path: pathlib.Path = BASELINE_JSON,
    snapshot_path: pathlib.Path = SNAPSHOT_JSON,
    decisions_path: pathlib.Path = DECISIONS_JSON,
) -> ContractAudit:
    """Audit live sources before accepting the reviewed snapshot and evidence."""

    baseline = load_snapshot(baseline_path)
    accepted = load_snapshot(snapshot_path)
    live = collect_repository_snapshot(repo_root, inventory_path)
    live_drift = compare_snapshots(accepted, live)
    if live_drift:
        details = "\n  ".join(delta.key for delta in live_drift)
        raise ContractInputError(
            "maintained upstream facts differ from the accepted snapshot; "
            "write review decisions before updating it:\n  " + details
        )
    manifest = load_review_manifest(decisions_path)
    return audit_review_manifest(baseline, accepted, manifest, repo_root=repo_root)


def _reference_snapshot(snapshot_path: pathlib.Path, baseline_path: pathlib.Path) -> ApiSnapshot:
    return load_snapshot(snapshot_path if snapshot_path.is_file() else baseline_path)


def _prepare_review_template_output(path: pathlib.Path) -> None:
    if path.exists():
        raise ContractInputError(
            f"refusing to overwrite existing review work at {path}; "
            "move or remove it explicitly after preserving any decisions"
        )
    path.parent.mkdir(parents=True, exist_ok=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="audit checked-in facts and decisions")
    mode.add_argument(
        "--write-review-template",
        action="store_true",
        help="write pending decisions for live drift",
    )
    mode.add_argument(
        "--update-snapshot",
        action="store_true",
        help="accept live facts after every delta has a reviewed decision",
    )
    parser.add_argument("--repo-root", type=pathlib.Path, default=REPO_ROOT)
    parser.add_argument("--inventory", type=pathlib.Path, default=INVENTORY_JSON)
    parser.add_argument("--baseline", type=pathlib.Path, default=BASELINE_JSON)
    parser.add_argument("--snapshot", type=pathlib.Path, default=SNAPSHOT_JSON)
    parser.add_argument("--decisions", type=pathlib.Path, default=DECISIONS_JSON)
    parser.add_argument(
        "--review-template",
        type=pathlib.Path,
        default=PENDING_DECISIONS_JSON,
        help="pending review file written by --write-review-template and accepted by --update-snapshot",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.check:
            audit = audit_repository_contract(
                repo_root=args.repo_root,
                inventory_path=args.inventory,
                baseline_path=args.baseline,
                snapshot_path=args.snapshot,
                decisions_path=args.decisions,
            )
            print(
                f"upstream contract OK: {len(audit.deltas)} reviewed deltas, "
                f"{len(audit.evidence_files)} evidence files"
            )
            return 0

        live = collect_repository_snapshot(args.repo_root, args.inventory)
        reference = _reference_snapshot(args.snapshot, args.baseline)
        if args.write_review_template:
            from upstream_contract_model import write_json

            _prepare_review_template_output(args.review_template)
            write_json(args.review_template, review_template(reference, live))
            print(
                f"wrote {len(compare_snapshots(reference, live))} pending decisions "
                f"to {args.review_template}"
            )
            return 0

        review_path = (
            args.review_template if args.review_template.is_file() else args.decisions
        )
        manifest = load_review_manifest(review_path)
        audit_review_manifest(reference, live, manifest, repo_root=args.repo_root)
        if args.snapshot.is_file() and compare_snapshots(reference, live):
            write_snapshot(args.baseline, reference)
        write_snapshot(args.snapshot, live)
        if review_path.resolve() != args.decisions.resolve():
            args.decisions.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(review_path, args.decisions)
        print(
            f"accepted {len(compare_snapshots(reference, live))} reviewed deltas "
            f"into {args.snapshot} and promoted {review_path} to {args.decisions}"
        )
        return 0
    except ContractInputError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


__all__ = [
    "ApiDelta",
    "ApiFact",
    "ApiSnapshot",
    "CLASSIFICATIONS",
    "ContractAudit",
    "ContractInputError",
    "DECISIONS_JSON",
    "EVIDENCE_KINDS",
    "FACT_KINDS",
    "INVENTORY_JSON",
    "JsonReader",
    "REPO_ROOT",
    "REVIEW_SCHEMA",
    "RUNTIME_SHAPED_KINDS",
    "ReviewEvidence",
    "ReviewGroup",
    "ReviewManifest",
    "SAFE_CLASSIFICATIONS",
    "SHA1_RE",
    "SNAPSHOT_SCHEMA",
    "SNAPSHOT_JSON",
    "BASELINE_JSON",
    "audit_repository_contract",
    "audit_review_manifest",
    "collect_generator_facts",
    "collect_repository_snapshot",
    "collect_rust_bindings_facts",
    "collect_snapshot_at_source_revisions",
    "compare_snapshots",
    "load_review_manifest",
    "load_snapshot",
    "main",
    "resolve_nested_source_revisions",
    "review_template",
    "write_snapshot",
]


if __name__ == "__main__":
    raise SystemExit(main())
