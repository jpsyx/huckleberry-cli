"""Disposable real Git repositories and an in-memory GitHub boundary."""
import subprocess
import tempfile
from pathlib import Path


def git(path, *arguments):
    return subprocess.check_output(["git", "-C", str(path), *arguments], text=True, stderr=subprocess.PIPE).strip()


class Fixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.origin = self.root / "origin.git"
        self.path = self.root / "checkout"
        subprocess.run(["git", "init", "--bare", "--initial-branch=main", str(self.origin)], check=True, capture_output=True)
        subprocess.run(["git", "clone", str(self.origin), str(self.path)], check=True, capture_output=True)
        git(self.path, "config", "user.name", "Test")
        git(self.path, "config", "user.email", "test@example.invalid")
        (self.path / "Cargo.toml").write_text('[package]\nname = "huckleberry-cli"\nversion = "0.47.2"\n')
        (self.path / "Cargo.lock").write_text('version = 4\n\n[[package]]\nname = "huckleberry-api"\nversion = "0.6.0"\n\n[[package]]\nname = "huckleberry-cli"\nversion = "0.47.2"\n')
        self.baseline = self.commit("chore: baseline")

    def commit(self, message, filename="example.txt", content="source\n"):
        (self.path / filename).write_text(content)
        git(self.path, "add", ".")
        git(self.path, "commit", "--allow-empty", "-m", message)
        source = git(self.path, "rev-parse", "HEAD")
        git(self.path, "push", "origin", "HEAD:main")
        return source

    def close(self):
        self.temporary.cleanup()


class FakeGitHub:
    repository = "jpsyx/huckleberry-cli"

    def __init__(self):
        self.releases = {}
        self.created_tags = []
        self.latest = None
        self.fail_after_create = False

    def release(self, tag):
        return self.releases.get(tag)

    def publish(self, record, body, make_latest):
        tag = "v" + record.version
        if tag not in self.releases:
            self.created_tags.append(tag)
            self.releases[tag] = {"tag_name": tag, "body": body, "draft": False, "prerelease": False}
        if make_latest:
            self.latest = tag
        if self.fail_after_create:
            self.fail_after_create = False
            raise ConnectionError("response lost")
        return self.releases[tag]
