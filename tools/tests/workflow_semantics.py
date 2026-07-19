"""Dependency-free structural reader for the repository's workflow contracts."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


_MAPPING_ENTRY = re.compile(r"(?P<key>[A-Za-z0-9_.-]+):(?:\s*(?P<value>.*))?\Z")
_BLOCK_SCALAR = re.compile(r"[>|][+-]?[1-9]?\Z")


class WorkflowSyntaxError(ValueError):
    """The maintained workflow uses YAML outside the supported contract subset."""


class _WorkflowParser:
    def __init__(self, source: str, *, label: str):
        self.lines = source.splitlines()
        self.label = label

    def parse(self) -> dict[str, Any]:
        value, index = self._mapping(0, 0)
        trailing = self._next_significant(index)
        if trailing < len(self.lines):
            self._fail(trailing, "unexpected trailing content")
        return value

    def _next_significant(self, index: int) -> int:
        while index < len(self.lines):
            stripped = self.lines[index].strip()
            if stripped and not stripped.startswith("#"):
                return index
            index += 1
        return index

    def _line(self, index: int) -> tuple[int, str]:
        raw = self.lines[index]
        prefix = raw[: len(raw) - len(raw.lstrip())]
        if "\t" in prefix:
            self._fail(index, "tabs are not valid indentation")
        return len(prefix), raw.lstrip()

    def _fail(self, index: int, message: str) -> None:
        raise WorkflowSyntaxError(f"{self.label}:{index + 1}: {message}")

    def _entry(self, index: int, text: str) -> tuple[str, str]:
        match = _MAPPING_ENTRY.fullmatch(text)
        if match is None:
            self._fail(index, f"expected a mapping entry, found {text!r}")
        return match.group("key"), (match.group("value") or "")

    def _mapping(self, index: int, indent: int) -> tuple[dict[str, Any], int]:
        value: dict[str, Any] = {}
        while True:
            index = self._next_significant(index)
            if index >= len(self.lines):
                return value, index
            line_indent, text = self._line(index)
            if line_indent < indent or (line_indent == indent and text.startswith("-")):
                return value, index
            if line_indent != indent:
                self._fail(index, f"expected indentation {indent}, found {line_indent}")
            key, raw_value = self._entry(index, text)
            if key in value:
                self._fail(index, f"duplicate mapping key {key!r}")
            item, index = self._value(index, indent, raw_value)
            value[key] = item

    def _value(self, index: int, indent: int, raw_value: str) -> tuple[Any, int]:
        raw_value = self._strip_inline_comment(raw_value).strip()
        if raw_value:
            if _BLOCK_SCALAR.fullmatch(raw_value) is not None:
                return self._block_scalar(index + 1, indent)
            return self._scalar(raw_value), index + 1

        child = self._next_significant(index + 1)
        if child >= len(self.lines):
            return None, child
        child_indent, child_text = self._line(child)
        if child_indent <= indent:
            return None, child
        if child_text.startswith("-"):
            return self._sequence(child, child_indent)
        return self._mapping(child, child_indent)

    def _sequence(self, index: int, indent: int) -> tuple[list[Any], int]:
        values: list[Any] = []
        while True:
            index = self._next_significant(index)
            if index >= len(self.lines):
                return values, index
            line_indent, text = self._line(index)
            if line_indent < indent or (
                line_indent == indent and not text.startswith("-")
            ):
                return values, index
            if line_indent != indent or not text.startswith("-"):
                self._fail(index, f"expected a sequence item at indentation {indent}")
            payload = text[1:].lstrip()
            if not payload:
                item, index = self._value(index, indent, "")
                values.append(item)
                continue

            match = _MAPPING_ENTRY.fullmatch(payload)
            if match is None:
                values.append(self._scalar(self._strip_inline_comment(payload).strip()))
                index += 1
                continue

            item_indent = indent + 2
            item: dict[str, Any] = {}
            key = match.group("key")
            first, index = self._value(index, item_indent, match.group("value") or "")
            item[key] = first
            continuation, index = self._mapping(index, item_indent)
            overlap = item.keys() & continuation.keys()
            if overlap:
                duplicate = sorted(overlap)[0]
                self._fail(index, f"duplicate sequence mapping key {duplicate!r}")
            item.update(continuation)
            values.append(item)

    def _block_scalar(self, index: int, indent: int) -> tuple[str, int]:
        parts: list[str] = []
        while index < len(self.lines):
            raw = self.lines[index]
            if not raw.strip():
                index += 1
                continue
            line_indent, text = self._line(index)
            if line_indent <= indent:
                break
            parts.append(text)
            index += 1
        return " ".join(parts), index

    def _scalar(self, value: str) -> Any:
        if value.startswith("[") and value.endswith("]"):
            body = value[1:-1].strip()
            if not body:
                return []
            return [self._scalar(part.strip()) for part in body.split(",")]
        if value == "{}":
            return {}
        if value in {"null", "Null", "NULL", "~"}:
            return None
        if value.casefold() == "true":
            return True
        if value.casefold() == "false":
            return False
        if re.fullmatch(r"-?(?:0|[1-9][0-9]*)", value):
            return int(value)
        if value.startswith('"') and value.endswith('"'):
            try:
                return json.loads(value)
            except json.JSONDecodeError:
                pass
        if value.startswith("'") and value.endswith("'"):
            return value[1:-1].replace("''", "'")
        return value

    @staticmethod
    def _strip_inline_comment(value: str) -> str:
        single = False
        double = False
        for index, character in enumerate(value):
            if character == "'" and not double:
                single = not single
            elif character == '"' and not single:
                double = not double
            elif (
                character == "#"
                and not single
                and not double
                and (index == 0 or value[index - 1].isspace())
            ):
                return value[:index]
        return value


def load_workflow(path: Path) -> dict[str, Any]:
    """Load the workflow subset needed by semantic contract tests."""
    path = Path(path)
    return _WorkflowParser(
        path.read_text(encoding="utf-8"), label=path.as_posix()
    ).parse()


def require_mapping(value: Any, label: str) -> dict[str, Any]:
    """Return a typed workflow mapping or fail with contract context."""
    if not isinstance(value, dict):
        raise AssertionError(f"{label} must be a mapping, found {type(value).__name__}")
    return value


def workflow_jobs(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Return every job as a mapping while preserving workflow order."""
    jobs = require_mapping(document.get("jobs"), "jobs")
    return {
        job_id: require_mapping(job, f"jobs.{job_id}")
        for job_id, job in jobs.items()
    }


def workflow_call_inputs(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Return a reusable workflow's declared input mappings."""
    triggers = require_mapping(document.get("on"), "on")
    call = require_mapping(triggers.get("workflow_call"), "on.workflow_call")
    inputs = require_mapping(call.get("inputs"), "on.workflow_call.inputs")
    return {
        name: require_mapping(specification, f"on.workflow_call.inputs.{name}")
        for name, specification in inputs.items()
    }


def job_dependencies(job: dict[str, Any]) -> tuple[str, ...]:
    """Normalize the scalar and flow-sequence forms accepted by ``needs``."""
    needs = job.get("needs", [])
    if isinstance(needs, str):
        return (needs,)
    if isinstance(needs, list) and all(isinstance(item, str) for item in needs):
        return tuple(needs)
    raise AssertionError(f"job needs must be a string or string list, found {needs!r}")


def named_step(job: dict[str, Any], name: str) -> dict[str, Any]:
    """Return one uniquely named workflow step."""
    steps = job.get("steps")
    if not isinstance(steps, list):
        raise AssertionError("job steps must be a sequence")
    matches = [
        require_mapping(step, f"step {name!r}")
        for step in steps
        if isinstance(step, dict) and step.get("name") == name
    ]
    if len(matches) != 1:
        raise AssertionError(f"expected one step named {name!r}, found {len(matches)}")
    return matches[0]
