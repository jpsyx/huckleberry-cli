"""Git operations for immutable release snapshots and version-only main updates."""
import json
import re
import subprocess
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path

from .versions import parse_version, replace_lock_version, replace_manifest_version

FILES = {"Cargo.toml": replace_manifest_version, "Cargo.lock": replace_lock_version}
BOT = ["-c", "user.name=github-actions[bot]", "-c",
       "user.email=41898282+github-actions[bot]@users.noreply.github.com",
       "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"]


@dataclass(frozen=True)
class Push:
    """The source range from one GitHub push event."""
    before: str
    source: str
    run_id: int


@dataclass(frozen=True)
class ReleaseRecord:
    """Durable reservation stored in an annotated tag."""
    version: str
    before: str
    source: str
    commit: str
    run_id: int


class Repository:
    """A disposable workflow checkout; all remote writes use its credentials."""
    def __init__(self, path: Path):
        self.path = Path(path).resolve()

    def git(self, *arguments: str) -> str:
        result = subprocess.run(["git", "-C", str(self.path), *arguments],
                                text=True, capture_output=True, check=True)
        return result.stdout.strip()

    def contents(self, commit: str, filename: str) -> str:
        return subprocess.check_output(["git", "-C", str(self.path), "show",
                                        f"{commit}:{filename}"], text=True)

    def fetch(self) -> None:
        self.git("fetch", "--quiet", "origin", "refs/heads/main:refs/remotes/origin/main", "--tags")

    def ancestor(self, earlier: str, later: str) -> bool:
        result = subprocess.run(["git", "-C", str(self.path), "merge-base", "--is-ancestor", earlier, later],
                                capture_output=True)
        if result.returncode not in (0, 1):
            raise ValueError("cannot resolve release source history")
        return result.returncode == 0

    def validate_push(self, push: Push) -> None:
        for sha in (push.before, push.source):
            if not re.fullmatch(r"[0-9a-f]{40}", sha) or sha == "0" * 40:
                raise ValueError("release requires existing, full before/after commit SHAs")
        if not self.ancestor(push.before, push.source) or not self.ancestor(push.source, "origin/main"):
            raise ValueError("main history was rewritten; release range is not an ancestor chain")

    def changes(self, push: Push) -> list[tuple[str, str]]:
        self.validate_push(push)
        records = self.records()
        changes = []
        for commit in self.git("rev-list", "--reverse", f"{push.before}..{push.source}").splitlines():
            message = self.git("show", "-s", "--format=%B", commit)
            if not self.generated(commit, message, records):
                changes.append((commit, message))
        return changes

    def messages(self, push: Push) -> list[str]:
        return [message for _, message in self.changes(push)]

    def version_only(self, parent: str, commit: str, version: str) -> bool:
        changed = set(self.git("diff", "--name-only", parent, commit).splitlines())
        return bool(changed) and changed <= FILES.keys() and all(
            replace(self.contents(parent, filename), version) == self.contents(commit, filename)
            for filename, replace in FILES.items())

    def generated(self, commit: str, message: str, records: list[ReleaseRecord]) -> bool:
        for record in records:
            if commit == record.commit:
                return True
            if message == self.commit_message(record):
                parent = self.git("rev-parse", f"{commit}^")
                if self.version_only(parent, commit, record.version):
                    return True
        return False

    def validate(self, record: ReleaseRecord) -> None:
        parse_version(record.version)
        self.validate_push(Push(record.before, record.source, record.run_id))
        if self.git("rev-parse", f"{record.commit}^") != record.source:
            raise ValueError("release tag parent does not match its source metadata")
        if not self.version_only(record.source, record.commit, record.version):
            raise ValueError("release tag is not an exact source plus CLI version update")

    def records(self) -> list[ReleaseRecord]:
        records = []
        for tag in self.git("tag", "--list", "v0.*").splitlines():
            try:
                parse_version(tag)
            except ValueError:
                continue
            metadata = json.loads(self.git("for-each-ref", "--format=%(contents)", f"refs/tags/{tag}"))
            if metadata.pop("schema", None) != 1 or tag != "v" + metadata.get("version", ""):
                raise ValueError(f"unrecognized release metadata on {tag}")
            record = ReleaseRecord(commit=self.git("rev-parse", f"{tag}^{{commit}}"), **metadata)
            self.validate(record)
            records.append(record)
        return sorted(records, key=lambda record: parse_version(record.version))

    @staticmethod
    def commit_message(record: ReleaseRecord) -> str:
        return f"chore(release): v{record.version}\n\nRelease-Source: {record.source}"

    def write_version(self, version: str) -> None:
        for filename, replace in FILES.items():
            path = self.path / filename
            path.write_text(replace(path.read_text(), version))

    def prepare(self, push: Push, version: str) -> ReleaseRecord:
        self.validate_push(push)
        if self.git("status", "--porcelain"):
            raise ValueError("release preparation needs a clean disposable checkout")
        self.git("checkout", "--quiet", "--detach", push.source)
        self.write_version(version)
        provisional = ReleaseRecord(version, push.before, push.source, "", push.run_id)
        self.git("add", *FILES)
        self.git(*BOT, "commit", "--quiet", "-m", self.commit_message(provisional))
        record = ReleaseRecord(version, push.before, push.source, self.git("rev-parse", "HEAD"), push.run_id)
        self.validate(record)
        metadata = asdict(record)
        del metadata["commit"]
        metadata["schema"] = 1
        self.git(*BOT, "tag", "-a", "v" + version, record.commit, "-m", json.dumps(metadata, sort_keys=True))
        return record

    def push_tag(self, record: ReleaseRecord) -> None:
        tag = "refs/tags/v" + record.version
        for attempt in range(5):
            try:
                self.git("push", "origin", tag)
                return
            except subprocess.CalledProcessError:
                remote = self.git("ls-remote", "origin", tag)
                if remote and remote.split()[0] == self.git("rev-parse", tag):
                    return
                if remote or attempt == 4:
                    raise
                time.sleep(min(2 ** attempt, 30))

    def sync_commit(self, record: ReleaseRecord, head: str) -> str:
        if head == record.source:
            return record.commit
        with tempfile.TemporaryDirectory(prefix="huckleberry-release-") as temporary:
            checkout = Path(temporary) / "checkout"
            subprocess.run(["git", "clone", "--quiet", "--shared", "--no-checkout", str(self.path), str(checkout)],
                           check=True, capture_output=True)
            sync = Repository(checkout)
            sync.git("checkout", "--quiet", "--detach", head)
            sync.write_version(record.version)
            sync.git("add", *FILES)
            sync.git(*BOT, "commit", "--quiet", "-m", self.commit_message(record))
            commit = sync.git("rev-parse", "HEAD")
            if not sync.version_only(head, commit, record.version):
                raise ValueError("main synchronization changed more than the version")
            self.git("fetch", "--quiet", str(checkout), commit)
            return commit

    def sync_main(self, record: ReleaseRecord) -> None:
        for attempt in range(5):
            self.fetch()
            head = self.git("rev-parse", "origin/main")
            current = re.search(r'^version = "([^"]+)"', self.contents(head, "Cargo.toml"), re.MULTILINE)
            if not current:
                raise ValueError("main has no CLI package version")
            if parse_version(current[1]) >= parse_version(record.version):
                return
            commit = self.sync_commit(record, head)
            try:
                self.git("push", "origin", f"{commit}:refs/heads/main")
                return
            except subprocess.CalledProcessError:
                self.fetch()
                if self.git("rev-parse", "origin/main") == head or attempt == 4:
                    raise
                time.sleep(min(2 ** attempt, 30))
