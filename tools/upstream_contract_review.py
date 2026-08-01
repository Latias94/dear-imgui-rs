"""Snapshot comparison and review-evidence validation for upstream contracts."""

from __future__ import annotations

import pathlib
import re
from typing import Any

from upstream_contract_model import (
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
    ReviewEvidence,
    ReviewGroup,
    ReviewManifest,
    json_array,
    json_mapping,
    normalized_posix_relative_path,
    object_exact,
    object_with_optional,
    read_json,
    string,
    string_list,
    write_json,
)


def load_snapshot(path: pathlib.Path) -> ApiSnapshot:
    raw = object_exact(
        read_json(path), {"schema", "source_revisions", "facts"}, str(path)
    )
    if raw["schema"] != SNAPSHOT_SCHEMA:
        raise ContractInputError(
            f"{path}.schema must be {SNAPSHOT_SCHEMA!r}, got {raw['schema']!r}"
        )
    revisions_raw = json_mapping(raw["source_revisions"], f"{path}.source_revisions")
    revisions: dict[str, str] = {}
    for key, value in revisions_raw.items():
        revision = string(value, f"{path}.source_revisions.{key}")
        if SHA1_RE.fullmatch(revision) is None:
            raise ContractInputError(
                f"{path}.source_revisions.{key} must be a full lowercase SHA-1"
            )
        revisions[key] = revision

    facts: list[ApiFact] = []
    seen: set[str] = set()
    for index, raw_fact in enumerate(json_array(raw["facts"], f"{path}.facts")):
        fact_data = object_exact(
            raw_fact,
            {"id", "source", "kind", "name", "value"},
            f"{path}.facts[{index}]",
        )
        source = string(fact_data["source"], f"{path}.facts[{index}].source")
        kind = string(fact_data["kind"], f"{path}.facts[{index}].kind")
        name = string(fact_data["name"], f"{path}.facts[{index}].name")
        if kind not in FACT_KINDS:
            raise ContractInputError(f"{path}.facts[{index}].kind is unsupported: {kind!r}")
        fact = ApiFact(source, kind, name, fact_data["value"])
        stored_id = string(fact_data["id"], f"{path}.facts[{index}].id")
        if stored_id != fact.id:
            raise ContractInputError(
                f"{path}.facts[{index}].id is {stored_id!r}; expected {fact.id!r}"
            )
        if fact.id in seen:
            raise ContractInputError(f"{path} contains duplicate fact {fact.id!r}")
        seen.add(fact.id)
        facts.append(fact)
    if [fact.id for fact in facts] != sorted(fact.id for fact in facts):
        raise ContractInputError(f"{path}.facts must be sorted by id")
    return ApiSnapshot(revisions, tuple(facts))


def write_snapshot(path: pathlib.Path, snapshot: ApiSnapshot) -> None:
    write_json(path, snapshot.as_json())


def compare_snapshots(baseline: ApiSnapshot, candidate: ApiSnapshot) -> tuple[ApiDelta, ...]:
    """Return every changed fact and source pin in deterministic order."""

    deltas: list[ApiDelta] = []
    revision_keys = sorted(set(baseline.source_revisions) | set(candidate.source_revisions))
    for key in revision_keys:
        before = baseline.source_revisions.get(key)
        after = candidate.source_revisions.get(key)
        if before == after:
            continue
        if before is None:
            operation = "added"
        elif after is None:
            operation = "removed"
        else:
            operation = "changed"
        deltas.append(
            ApiDelta(operation, f"revision:{key}", key, "source-pin", key, before, after)
        )

    baseline_facts = {fact.id: fact for fact in baseline.facts}
    candidate_facts = {fact.id: fact for fact in candidate.facts}
    for fact_id in sorted(set(baseline_facts) | set(candidate_facts)):
        before_fact = baseline_facts.get(fact_id)
        after_fact = candidate_facts.get(fact_id)
        if before_fact is None:
            assert after_fact is not None
            deltas.append(
                ApiDelta(
                    "added",
                    fact_id,
                    after_fact.source,
                    after_fact.kind,
                    after_fact.name,
                    None,
                    after_fact.value,
                )
            )
        elif after_fact is None:
            deltas.append(
                ApiDelta(
                    "removed",
                    fact_id,
                    before_fact.source,
                    before_fact.kind,
                    before_fact.name,
                    before_fact.value,
                    None,
                )
            )
        elif before_fact.value != after_fact.value:
            deltas.append(
                ApiDelta(
                    "changed",
                    fact_id,
                    after_fact.source,
                    after_fact.kind,
                    after_fact.name,
                    before_fact.value,
                    after_fact.value,
                )
            )
    return tuple(sorted(deltas, key=lambda item: item.key))


def load_review_manifest(path: pathlib.Path) -> ReviewManifest:
    raw = object_exact(
        read_json(path),
        {"schema", "baseline_sha256", "candidate_sha256", "groups"},
        str(path),
    )
    if raw["schema"] != REVIEW_SCHEMA:
        raise ContractInputError(
            f"{path}.schema must be {REVIEW_SCHEMA!r}, got {raw['schema']!r}"
        )
    baseline_hash = string(raw["baseline_sha256"], f"{path}.baseline_sha256")
    candidate_hash = string(raw["candidate_sha256"], f"{path}.candidate_sha256")
    for name, digest in (
        ("baseline_sha256", baseline_hash),
        ("candidate_sha256", candidate_hash),
    ):
        if re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            raise ContractInputError(f"{path}.{name} must be a lowercase SHA-256")

    groups: list[ReviewGroup] = []
    names: set[str] = set()
    for index, raw_group in enumerate(json_array(raw["groups"], f"{path}.groups")):
        context = f"{path}.groups[{index}]"
        group = object_with_optional(
            raw_group,
            {"name", "status", "classification", "rationale", "items"},
            {"safe_api", "evidence"},
            context,
        )
        name = string(group["name"], f"{context}.name")
        if name in names:
            raise ContractInputError(f"{path} contains duplicate review group {name!r}")
        names.add(name)
        evidence: list[ReviewEvidence] = []
        for evidence_index, raw_evidence in enumerate(
            json_array(group.get("evidence", []), f"{context}.evidence")
        ):
            evidence_context = f"{context}.evidence[{evidence_index}]"
            evidence_data = object_exact(
                raw_evidence, {"kind", "path", "contains"}, evidence_context
            )
            kind = string(evidence_data["kind"], f"{evidence_context}.kind")
            if kind not in EVIDENCE_KINDS:
                raise ContractInputError(
                    f"{evidence_context}.kind must be one of {sorted(EVIDENCE_KINDS)}"
                )
            evidence.append(
                ReviewEvidence(
                    kind,
                    normalized_posix_relative_path(
                        evidence_data["path"], f"{evidence_context}.path"
                    ),
                    string(evidence_data["contains"], f"{evidence_context}.contains"),
                )
            )
        groups.append(
            ReviewGroup(
                name=name,
                status=string(group["status"], f"{context}.status"),
                classification=string(
                    group["classification"], f"{context}.classification", allow_empty=True
                ),
                rationale=string(
                    group["rationale"], f"{context}.rationale", allow_empty=True
                ),
                items=string_list(group["items"], f"{context}.items"),
                safe_api=string_list(
                    group.get("safe_api", []), f"{context}.safe_api", allow_empty=True
                ),
                evidence=tuple(evidence),
            )
        )
    return ReviewManifest(baseline_hash, candidate_hash, tuple(groups))


def _resolve_evidence_file(repo_root: pathlib.Path, evidence: ReviewEvidence) -> pathlib.Path:
    """Validate a manifest evidence path again and resolve it inside the repository."""

    relative = normalized_posix_relative_path(evidence.path.as_posix(), "review evidence path")
    try:
        resolved_root = repo_root.resolve(strict=True)
        candidate = resolved_root.joinpath(*relative.parts)
        resolved_candidate = candidate.resolve(strict=True)
    except (OSError, UnicodeError) as error:
        raise ContractInputError(
            f"review evidence file {relative.as_posix()} cannot be resolved: {error}"
        ) from error
    if not resolved_candidate.is_relative_to(resolved_root):
        raise ContractInputError(
            f"review evidence file {relative.as_posix()} resolves outside the repository"
        )
    if not resolved_candidate.is_file():
        raise ContractInputError(
            f"review evidence file {relative.as_posix()} is not a regular file"
        )
    return resolved_candidate


def audit_review_manifest(
    baseline: ApiSnapshot,
    candidate: ApiSnapshot,
    manifest: ReviewManifest,
    *,
    repo_root: pathlib.Path,
) -> ContractAudit:
    """Fail closed unless one reviewed group owns every upstream delta."""

    violations: list[str] = []
    if manifest.baseline_sha256 != baseline.digest:
        violations.append(
            "review baseline hash is stale: "
            f"expected {baseline.digest}, got {manifest.baseline_sha256}"
        )
    if manifest.candidate_sha256 != candidate.digest:
        violations.append(
            "review candidate hash is stale: "
            f"expected {candidate.digest}, got {manifest.candidate_sha256}"
        )

    deltas = compare_snapshots(baseline, candidate)
    delta_by_key = {delta.key: delta for delta in deltas}
    reviewed: dict[str, str] = {}
    evidence_files: set[pathlib.Path] = set()
    for group in manifest.groups:
        if group.status != "reviewed":
            violations.append(
                f"review group {group.name!r} has status {group.status!r}; expected 'reviewed'"
            )
        if group.classification not in CLASSIFICATIONS:
            violations.append(
                f"review group {group.name!r} has invalid classification "
                f"{group.classification!r}; expected one of {sorted(CLASSIFICATIONS)}"
            )
        if not group.rationale.strip():
            violations.append(f"review group {group.name!r} has no rationale")
        group_deltas: list[ApiDelta] = []
        for item in group.items:
            if item in reviewed:
                violations.append(
                    f"review item {item!r} appears in both {reviewed[item]!r} and {group.name!r}"
                )
            else:
                reviewed[item] = group.name
            delta = delta_by_key.get(item)
            if delta is None:
                violations.append(
                    f"review group {group.name!r} contains stale or unknown item {item!r}"
                )
            else:
                group_deltas.append(delta)

        safe = group.classification in SAFE_CLASSIFICATIONS
        if safe:
            if not group.safe_api:
                violations.append(
                    f"safe review group {group.name!r} must name at least one safe Rust API"
                )
            evidence_kinds = {item.kind for item in group.evidence}
            if "compile" not in evidence_kinds:
                violations.append(
                    f"safe review group {group.name!r} lacks compile usability evidence"
                )
            needs_runtime = any(
                delta.operation in {"added", "changed"}
                and delta.kind in RUNTIME_SHAPED_KINDS
                for delta in group_deltas
            )
            if needs_runtime and "runtime" not in evidence_kinds:
                violations.append(
                    f"safe review group {group.name!r} lacks runtime usability evidence"
                )
        elif group.safe_api or group.evidence:
            violations.append(
                f"non-safe review group {group.name!r} must not claim safe API or evidence"
            )

        for evidence in group.evidence:
            try:
                path = _resolve_evidence_file(repo_root, evidence)
            except ContractInputError as error:
                violations.append(f"review group {group.name!r} {error}")
                continue
            evidence_files.add(path)
            try:
                contents = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                violations.append(
                    f"review group {group.name!r} evidence file {evidence.path} "
                    f"cannot be read: {error}"
                )
                continue
            if evidence.contains not in contents:
                violations.append(
                    f"review group {group.name!r} evidence marker "
                    f"{evidence.contains!r} is absent from {evidence.path}"
                )

    missing = sorted(set(delta_by_key) - set(reviewed))
    if missing:
        violations.append("unreviewed upstream deltas:\n  " + "\n  ".join(missing))
    if violations:
        raise ContractInputError("upstream contract review failed:\n- " + "\n- ".join(violations))
    return ContractAudit(deltas, len(reviewed), tuple(sorted(evidence_files)))


def review_template(baseline: ApiSnapshot, candidate: ApiSnapshot) -> dict[str, Any]:
    groups = []
    for delta in compare_snapshots(baseline, candidate):
        groups.append(
            {
                "name": delta.key,
                "status": "pending",
                "classification": "",
                "rationale": "",
                "items": [delta.key],
            }
        )
    return {
        "schema": REVIEW_SCHEMA,
        "baseline_sha256": baseline.digest,
        "candidate_sha256": candidate.digest,
        "groups": groups,
    }
