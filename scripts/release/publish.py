"""One push becomes one reserved version, release, and main version update."""
from typing import Callable, Optional

from .repository import Push, ReleaseRecord, Repository
from .versions import next_version, parse_version

BASELINE = "0.47.2"


def description(repository: Repository, name: str, push: Push) -> str:
    """Useful automatic descriptions for direct commits as well as PR merges."""
    base = f"https://github.com/{name}"
    lines = ["## Commits in this push", ""]
    for commit, message in repository.changes(push):
        subject = message.splitlines()[0] if message else "Empty commit message"
        lines.append(f"- {subject} ([{commit[:7]}]({base}/commit/{commit}))")
    lines.extend(["", f"Source: `{push.source}`", "",
                  f"[Compare this push]({base}/compare/{push.before}...{push.source})", ""])
    return "\n".join(lines)


def publish_push(repository: Repository, github, push: Push,
                 verify: Callable[[ReleaseRecord], None]) -> Optional[ReleaseRecord]:
    """Resume partial publication by source identity without moving existing tags."""
    repository.fetch()
    repository.validate_push(push)
    records = repository.records()
    existing = [record for record in records if record.source == push.source]
    if len(existing) > 1:
        raise ValueError("multiple release reservations for the same push")
    if existing:
        record = existing[0]
        if record.before != push.before:
            raise ValueError("release reservation does not match the push range")
        repository.git("checkout", "--quiet", "--detach", record.commit)
    else:
        if any(record.source != push.source and repository.ancestor(push.source, record.source) for record in records):
            print("This unreserved push was superseded by a newer released source.")
            return None
        current = records[-1].version if records else BASELINE
        record = repository.prepare(push, next_version(current, repository.messages(push)))
    verify(record)
    repository.push_tag(record)
    latest = not any(parse_version(other.version) > parse_version(record.version) for other in records)
    tag = "v" + record.version
    release = github.release(tag)
    if release is None:
        release = github.publish(record, description(repository, github.repository, push), latest)
    if release.get("tag_name") != tag or release.get("draft") or release.get("prerelease"):
        raise ValueError("existing GitHub release does not match the published version")
    repository.sync_main(record)
    return record
