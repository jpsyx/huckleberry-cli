"""Actions entry points. All commit text is passed as data, never shell code."""
import argparse
import json
import os
import subprocess
from pathlib import Path

from .github import GitHub
from .publish import publish_push
from .repository import Push, ReleaseRecord, Repository


def verify_build(repository: Repository, record: ReleaseRecord) -> None:
    """Verify the actual tagged build version without permitting lockfile drift."""
    result = subprocess.run(["cargo", "run", "--locked", "--", "--version"],
                            cwd=repository.path, text=True, capture_output=True, check=True)
    if result.stdout.strip() != "huckleberry-cli " + record.version:
        raise ValueError("compiled version differs from the release tag")
    if repository.git("status", "--porcelain"):
        raise ValueError("release verification changed the checkout")


def main() -> None:
    """Validate workflow context before any publication operation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["publish"])
    parser.parse_args()
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("GITHUB_REF") != "refs/heads/main":
        raise ValueError("release entry points run only in a main-branch GitHub Actions push")
    if os.environ.get("GITHUB_EVENT_NAME") != "push":
        raise ValueError("release entry points require a push event")
    run_id = int(os.environ["GITHUB_RUN_ID"])
    github = GitHub(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_TOKEN"])
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    if event["ref"] != "refs/heads/main" or event["after"] != os.environ["GITHUB_SHA"] or event.get("forced"):
        raise ValueError("release event does not match a normal main push")
    repository = Repository(Path.cwd())
    record = publish_push(repository, github, Push(event["before"], event["after"], run_id),
                          lambda record: verify_build(repository, record))
    if record:
        print(f"Published https://github.com/{github.repository}/releases/tag/v{record.version}")


if __name__ == "__main__":
    main()
