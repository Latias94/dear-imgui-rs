"""Shared data model and strict JSON helpers for upstream contract audits."""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
from dataclasses import dataclass
from typing import Any, Callable, Mapping


SNAPSHOT_SCHEMA = "dear-imgui-upstream-contract-snapshot-v2"
REVIEW_SCHEMA = "dear-imgui-upstream-contract-review-v1"
SHA1_RE = re.compile(r"[0-9a-f]{40}")
FACT_KINDS = frozenset(
    {"function", "constant", "enum", "enum-variant", "field", "layout", "typedef"}
)
CLASSIFICATIONS = frozenset(
    {"safe-alias", "safe-wrapper", "raw-only", "rejected", "internal"}
)
SAFE_CLASSIFICATIONS = frozenset({"safe-alias", "safe-wrapper"})
RUNTIME_SHAPED_KINDS = frozenset({"function", "field", "layout"})
EVIDENCE_KINDS = frozenset({"compile", "runtime"})


class ContractInputError(ValueError):
    """Raised when a checked-in contract artifact is malformed or incomplete."""


@dataclass(frozen=True)
class ApiFact:
    source: str
    kind: str
    name: str
    value: Any

    @property
    def id(self) -> str:
        return f"{self.source}:{self.kind}:{self.name}"

    def as_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "source": self.source,
            "kind": self.kind,
            "name": self.name,
            "value": self.value,
        }


@dataclass(frozen=True)
class ApiSnapshot:
    source_revisions: Mapping[str, str]
    facts: tuple[ApiFact, ...]

    def as_json(self) -> dict[str, Any]:
        return {
            "schema": SNAPSHOT_SCHEMA,
            "source_revisions": dict(sorted(self.source_revisions.items())),
            "facts": [fact.as_json() for fact in sorted(self.facts, key=lambda item: item.id)],
        }

    @property
    def digest(self) -> str:
        payload = json.dumps(
            self.as_json(), sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode("utf-8")
        return hashlib.sha256(payload).hexdigest()


@dataclass(frozen=True)
class ApiDelta:
    operation: str
    item_id: str
    source: str
    kind: str
    name: str
    before: Any
    after: Any

    @property
    def key(self) -> str:
        return f"{self.operation}:{self.item_id}"


@dataclass(frozen=True)
class ReviewEvidence:
    kind: str
    path: pathlib.PurePosixPath
    contains: str


@dataclass(frozen=True)
class ReviewGroup:
    name: str
    status: str
    classification: str
    rationale: str
    items: tuple[str, ...]
    safe_api: tuple[str, ...]
    evidence: tuple[ReviewEvidence, ...]


@dataclass(frozen=True)
class ReviewManifest:
    baseline_sha256: str
    candidate_sha256: str
    groups: tuple[ReviewGroup, ...]


@dataclass(frozen=True)
class ContractAudit:
    deltas: tuple[ApiDelta, ...]
    reviewed_items: int
    evidence_files: tuple[pathlib.Path, ...]


JsonReader = Callable[[pathlib.Path], Any]


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ContractInputError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def read_json(path: pathlib.Path) -> Any:
    try:
        with path.open(encoding="utf-8") as input_file:
            return json.load(input_file, object_pairs_hook=reject_duplicate_keys)
    except ContractInputError:
        raise
    except (OSError, json.JSONDecodeError) as error:
        raise ContractInputError(f"could not read {path}: {error}") from error


def write_json(path: pathlib.Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, indent=2, ensure_ascii=False, sort_keys=False) + "\n",
        encoding="utf-8",
    )


def object_exact(value: Any, keys: set[str], context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractInputError(f"{context} must be an object")
    actual = set(value)
    if actual != keys:
        missing = sorted(keys - actual)
        unexpected = sorted(actual - keys)
        details: list[str] = []
        if missing:
            details.append(f"missing {missing}")
        if unexpected:
            details.append(f"unexpected {unexpected}")
        raise ContractInputError(f"{context} has invalid keys: {', '.join(details)}")
    return value


def object_with_optional(
    value: Any, required: set[str], optional: set[str], context: str
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractInputError(f"{context} must be an object")
    actual = set(value)
    missing = required - actual
    unexpected = actual - required - optional
    if missing or unexpected:
        details: list[str] = []
        if missing:
            details.append(f"missing {sorted(missing)}")
        if unexpected:
            details.append(f"unexpected {sorted(unexpected)}")
        raise ContractInputError(f"{context} has invalid keys: {', '.join(details)}")
    return value


def string(value: Any, context: str, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value.strip()):
        qualifier = "a string" if allow_empty else "a non-empty string"
        raise ContractInputError(f"{context} must be {qualifier}")
    return value


def string_list(value: Any, context: str, *, allow_empty: bool = False) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise ContractInputError(f"{context} must be an array")
    result = tuple(string(item, f"{context}[{index}]") for index, item in enumerate(value))
    if not allow_empty and not result:
        raise ContractInputError(f"{context} must not be empty")
    if len(result) != len(set(result)):
        raise ContractInputError(f"{context} contains duplicate values")
    return result


def json_mapping(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ContractInputError(f"{context} must be an object with string keys")
    return value


def json_array(value: Any, context: str) -> list[Any]:
    if not isinstance(value, list):
        raise ContractInputError(f"{context} must be an array")
    return value


def normalized_posix_relative_path(value: Any, context: str) -> pathlib.PurePosixPath:
    """Return one canonical repository-relative POSIX path or reject it.

    This deliberately does not defer to the host filesystem: review manifests
    are cross-platform inputs, so Windows drive, UNC, and rooted syntax must be
    rejected on every host before a local ``Path`` can reinterpret it.
    """

    raw = string(value, context)
    if (
        "\\" in raw
        or raw.startswith("/")
        or raw.startswith("//")
        or re.match(r"^[A-Za-z]:", raw) is not None
    ):
        raise ContractInputError(
            f"{context} must be a normalized forward-slash repository-relative path"
        )
    parts = raw.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise ContractInputError(
            f"{context} must be a normalized forward-slash repository-relative path"
        )
    return pathlib.PurePosixPath(raw)
