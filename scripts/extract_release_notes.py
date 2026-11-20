#!/usr/bin/env python3
"""Extract release notes for a tagged version from CHANGELOG.md.

Follows Keep a Changelog headings such as `## [0.1.0] - 2026-11-20` or
`## [Unreleased]`. Falls back to `[Unreleased]` when no section matches the
requested version so dry-runs and early tags still get meaningful notes.
"""

from __future__ import annotations

import pathlib
import re
import sys

HEADING_RE = re.compile(r"^## \[([^\]]+)\](?:\s*-\s*.*)?\s*$")


def extract_section(changelog: str, version: str) -> str:
    """Return the markdown body under `## [<version>]` (without the heading)."""
    normalized = version.lstrip("vV")
    sections: dict[str, list[str]] = {}
    current: str | None = None

    for line in changelog.splitlines():
        match = HEADING_RE.match(line)
        if match:
            current = match.group(1).strip()
            sections[current] = []
            continue
        if current is not None:
            # Stop before Keep a Changelog comparison link definitions at EOF.
            if re.match(r"^\[[^\]]+\]:\s+https?://", line):
                current = None
                continue
            sections[current].append(line)

    body = sections.get(normalized)
    if body is None:
        body = sections.get("Unreleased")
    if not body:
        return f"Release `{version}` of Trazo."

    return "\n".join(body).strip()


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        sys.stderr.write(
            f"Usage: {argv[0]} <tag-or-version> [path-to-CHANGELOG.md]\n"
        )
        return 2

    version = argv[1]
    changelog_path = pathlib.Path(argv[2]) if len(argv) == 3 else pathlib.Path("CHANGELOG.md")
    changelog = changelog_path.read_text(encoding="utf-8")
    notes = extract_section(changelog, version)
    print(notes)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
