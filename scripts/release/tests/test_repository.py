"""Exact-source tags and preservation of concurrent main changes."""
import unittest
from unittest.mock import patch
from dataclasses import replace
from scripts.release.repository import Push, Repository
from scripts.release.versions import next_version
from support import Fixture, git


class RepositoryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = Fixture()
        self.addCleanup(self.fixture.close)
        self.repo = Repository(self.fixture.path)
        source = self.fixture.commit("feat: version modal")
        self.push = Push(self.fixture.baseline, source, 1)
        self.repo.fetch()

    def test_tag_is_version_only_child_of_exact_source(self):
        record = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(record)
        self.assertEqual(git(self.fixture.path, "rev-parse", "v0.48.0^"), self.push.source)
        self.assertEqual(git(self.fixture.path, "diff", "--name-only", record.commit + "^", record.commit).splitlines(), ["Cargo.lock", "Cargo.toml"])
        self.assertEqual(self.repo.records(), [record])

    def test_main_sync_preserves_newer_source(self):
        record = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(record)
        git(self.fixture.path, "checkout", "main")
        newer = self.fixture.commit("fix: later push", "later.txt", "newer source is retained\n")
        self.repo.sync_main(record)
        self.repo.fetch()
        self.assertEqual(git(self.fixture.path, "show", "origin/main:later.txt"), "newer source is retained")
        self.assertEqual(git(self.fixture.path, "rev-parse", "origin/main^"), newer)
        self.assertIn('version = "0.48.0"', git(self.fixture.path, "show", "origin/main:Cargo.toml"))
        self.assertEqual(git(self.fixture.path, "rev-parse", "v0.48.0^"), self.push.source)

    def test_main_can_fast_forward_to_release_commit(self):
        record = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(record)
        self.repo.sync_main(record)
        self.repo.fetch()
        self.assertEqual(git(self.fixture.path, "rev-parse", "origin/main"), record.commit)

    def test_reused_tag_metadata_must_match_its_source(self):
        record = self.repo.prepare(self.push, "0.48.0")
        with self.assertRaises(ValueError):
            self.repo.validate(replace(record, source=self.fixture.baseline))

    def test_merge_range_includes_feature_commit(self):
        git(self.fixture.path, "checkout", "-b", "topic")
        (self.fixture.path / "feature.txt").write_text("feature")
        git(self.fixture.path, "add", ".")
        git(self.fixture.path, "commit", "-m", "feat: merged feature")
        git(self.fixture.path, "checkout", "main")
        git(self.fixture.path, "merge", "--no-ff", "topic", "-m", "Merge topic")
        source = git(self.fixture.path, "rev-parse", "HEAD")
        git(self.fixture.path, "push", "origin", "main")
        self.repo.fetch()
        messages = self.repo.messages(Push(self.push.source, source, 2))
        self.assertEqual(next_version("0.48.0", messages), "0.49.0")

    def test_rewritten_history_and_invalid_shas_are_rejected(self):
        with self.assertRaises(ValueError):
            self.repo.messages(Push(self.push.source, self.fixture.baseline, 2))
        with self.assertRaises(ValueError):
            self.repo.messages(Push("--all", self.push.source, 2))

    def test_generated_version_commits_are_excluded_but_similar_user_subjects_are_not(self):
        record = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(record)
        self.repo.sync_main(record)
        git(self.fixture.path, "checkout", "main")
        git(self.fixture.path, "merge", "--ff-only", record.commit)
        source = self.fixture.commit(self.repo.commit_message(record), "real-change.txt", "user change")
        self.repo.fetch()
        self.assertEqual(self.repo.messages(Push(self.push.source, source, 2)), [self.repo.commit_message(record)])

    def test_retries_when_main_advances_between_fetch_and_push(self):
        record = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(record)
        original = self.repo.git
        advanced = False
        def raced(*arguments):
            nonlocal advanced
            if arguments[:2] == ("push", "origin") and arguments[-1].endswith(":refs/heads/main") and not advanced:
                advanced = True
                git(self.fixture.path, "checkout", "main")
                self.fixture.commit("fix: concurrent", "concurrent.txt", "keep this")
            return original(*arguments)
        with patch.object(self.repo, "git", side_effect=raced), patch("scripts.release.repository.time.sleep"):
            self.repo.sync_main(record)
        self.repo.fetch()
        self.assertEqual(git(self.fixture.path, "show", "origin/main:concurrent.txt"), "keep this")
        self.assertIn('version = "0.48.0"', git(self.fixture.path, "show", "origin/main:Cargo.lock"))
