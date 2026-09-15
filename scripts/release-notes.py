#!/usr/bin/env python3
"""Prints the GitHub Release body for a tag.

    python3 scripts/release-notes.py v0.5.0 CHANGELOG.md

The version's own section comes from CHANGELOG.md; the standing advice below is
appended to it. Exits non-zero when the tag has no section, which fails the
release build on purpose: 0.5.0 shipped under 0.4.0's notes because the body
was hardcoded in the workflow, where no pull request ever thinks to update it.
"""
import re
import sys

FOOTER = """
**On first launch:** enter your organization's KFIRE server address, then
approve the device in the browser - no password needed.

**Security warning?** The installers aren't code-signed yet, so your OS may
warn about an "unknown publisher". The app is safe:
- Windows (SmartScreen): *More info* -> *Run anyway*
- macOS (Gatekeeper): right-click the app -> *Open* -> *Open*
- Linux: no warning
"""


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <tag> <changelog>")
    tag = sys.argv[1].lstrip("vV")
    text = open(sys.argv[2], encoding="utf-8").read()
    found = re.search(
        rf"^## {re.escape(tag)}\s*$(.*?)(?=^## |\Z)", text, re.M | re.S
    )
    if not found or not found.group(1).strip():
        sys.exit(f"CHANGELOG.md has no section for {tag}; add one before tagging")
    print(found.group(1).strip())
    print(FOOTER.rstrip())


if __name__ == "__main__":
    main()
