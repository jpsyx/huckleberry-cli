"""Pure version policy and formatting-preserving Cargo edits."""
import re
from typing import Sequence


def parse_version(value: str) -> tuple[int, int, int]:
    """Parse a stable pre-1.0 release version, allowing a tag's leading v."""
    match = re.fullmatch(r"v?(0)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value)
    if not match:
        raise ValueError(f"not a stable pre-1.0 version: {value!r}")
    return tuple(int(part) for part in match.groups())


def next_version(current: str, messages: Sequence[str]) -> str:
    """One minor bump for features/breaking changes; otherwise one patch."""
    major, minor, patch = parse_version(current)
    feature = re.compile(r"^feat(?:\([^\n)]+\))?!?:\s")
    breaking = re.compile(r"^[a-z]+(?:\([^\n)]+\))?!:\s")
    footer = re.compile(r"^BREAKING(?: CHANGE|-CHANGE):\s", re.MULTILINE)
    if any(feature.match(message) or breaking.match(message) or footer.search(message)
           for message in messages):
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def _replace(text: str, version: str, heading: str) -> str:
    parse_version(version)
    sections = list(re.finditer(r"(?ms)^" + re.escape(heading) + r"[^\n]*\n(.*?)(?=^\[|\Z)", text))
    matches = [section for section in sections
               if re.search(r'^name\s*=\s*"huckleberry-cli"\s*$', section[1], re.MULTILINE)]
    if len(matches) != 1:
        raise ValueError("expected exactly one huckleberry-cli package")
    section = matches[0]
    body, count = re.subn(r'(?m)^(version\s*=\s*)"[^"\n]+"',
                          lambda match: match[1] + f'"{version}"', section[1])
    if count != 1:
        raise ValueError("expected exactly one CLI package version")
    return text[:section.start(1)] + body + text[section.end(1):]


def replace_manifest_version(text: str, version: str) -> str:
    """Replace only the root CLI package's version, preserving formatting."""
    return _replace(text, version, "[package]")


def replace_lock_version(text: str, version: str) -> str:
    """Replace only the CLI lock entry, preserving dependencies and API version."""
    return _replace(text, version, "[[package]]")
