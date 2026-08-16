#!/usr/bin/env python3

# @dependency-start
# contract implementation
# responsibility Classifies release-watch roadmap invariants without treating historical or negative prose as unfinished work.
# upstream issue https://github.com/iwashita-nozomu/ptymark/issues/157 alert contract
# upstream issue https://github.com/iwashita-nozomu/ptymark/issues/160 schema-v2 false-positive correction
# downstream environment ../.github/workflows/ptymark-oss-release-watch.yml scheduled classification
# downstream implementation ../tests/tools/test_release_watch.py classification regressions
# @dependency-end

"""Deterministic classifiers used by the default-branch release watch."""

from __future__ import annotations

import argparse
import re
from collections.abc import Iterator
from pathlib import Path

_TASK = re.compile(
    r"^(?P<indent>[ \t]*)[-*+]\s*\[(?P<state>[ xX])\]\s*(?P<body>.*)$"
)
_LIST_ITEM = re.compile(r"^(?P<indent>[ \t]*)[-*+]\s+")
_HEADING = re.compile(r"^[ \t]{0,3}#{1,6}\s+")
_SCHEMA_V1_ASSIGNMENT = re.compile(
    r"(?<![A-Za-z0-9_])schema_version\s*=\s*1(?![0-9])"
)
_SCHEMA_V2_HEADING = re.compile(r"(?im)^[ \t]{0,3}##[ \t]+Schema v2[ \t]*$")


def _indent_width(indent: str) -> int:
    """Return a deterministic indentation width for Markdown list boundaries."""

    return sum(4 if character == "\t" else 1 for character in indent)


def unchecked_task_blocks(markdown: str) -> Iterator[str]:
    """Yield the text owned by each unchecked Markdown task.

    Continuation lines, including an indented fenced code block, remain attached to
    the task until a same-or-outer-level list item or a heading starts. Completed
    tasks and ordinary bullets are deliberately excluded: release-watch alerts
    represent unfinished work, not historical evidence or acceptance prose.
    """

    current_indent: int | None = None
    current_lines: list[str] = []

    def flush() -> str | None:
        nonlocal current_indent, current_lines
        if current_indent is None:
            return None
        block = "\n".join(current_lines)
        current_indent = None
        current_lines = []
        return block

    for line in markdown.splitlines():
        task = _TASK.match(line)
        if task is not None:
            block = flush()
            if block is not None:
                yield block
            if task.group("state") == " ":
                current_indent = _indent_width(task.group("indent"))
                current_lines = [task.group("body")]
            continue

        if current_indent is None:
            continue

        list_item = _LIST_ITEM.match(line)
        starts_peer_item = (
            list_item is not None
            and _indent_width(list_item.group("indent")) <= current_indent
        )
        if starts_peer_item or _HEADING.match(line):
            block = flush()
            if block is not None:
                yield block
            continue

        current_lines.append(line)

    block = flush()
    if block is not None:
        yield block


def schema_v2_roadmap_mismatch(issue_body: str, design: str) -> bool:
    """Return whether an actionable future schema-v1 contract conflicts with v2.

    A literal ``schema_version = 1`` is actionable only when it belongs to an
    unchecked Markdown task. This keeps the monitor sensitive to unfinished
    future-contract work while excluding completed migration evidence, historical
    examples, and negative acceptance statements.
    """

    if _SCHEMA_V2_HEADING.search(design) is None:
        return False
    return any(
        _SCHEMA_V1_ASSIGNMENT.search(block) is not None
        for block in unchecked_task_blocks(issue_body)
    )


def _read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise SystemExit(f"cannot read {path}: {error}") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    schema = subparsers.add_parser(
        "schema-v2-status",
        help="print mismatch when unfinished schema-v1 work conflicts with Schema v2",
    )
    schema.add_argument("issue_body", type=Path)
    schema.add_argument("design", type=Path)
    arguments = parser.parse_args(argv)

    if arguments.command == "schema-v2-status":
        mismatch = schema_v2_roadmap_mismatch(
            _read(arguments.issue_body), _read(arguments.design)
        )
        print("mismatch" if mismatch else "aligned")
        return 0

    parser.error(f"unsupported command: {arguments.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
