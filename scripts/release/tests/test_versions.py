"""Release policy and narrow Cargo edits, independent of GitHub."""
import unittest
import re
from pathlib import Path

from scripts.release.versions import (
    next_version, parse_version, replace_lock_version, replace_manifest_version,
)


class VersionTests(unittest.TestCase):
    def test_features_and_breaking_changes_select_minor_once(self):
        for message in ["feat: modal", "feat(tui): modal", "fix!: interface",
                        "refactor(cli)!: flags", "fix: flags\n\nBREAKING CHANGE: v",
                        "fix: flags\n\nBREAKING-CHANGE: v"]:
            with self.subTest(message=message):
                self.assertEqual(next_version("0.47.2", ["fix: other", message]), "0.48.0")

    def test_all_other_pushes_still_release(self):
        for messages in [[], ["fix: height"], ["docs: guide"], ["Update docs"],
                         ["docs: explain feat: headers"], ["fix: x\nmentions feat: y"]]:
            self.assertEqual(next_version("0.47.2", messages), "0.47.3")

    def test_parse_requires_pre_one_stable_semver(self):
        self.assertEqual(parse_version("v0.10.12"), (0, 10, 12))
        for value in ["1.0.0", "0.1", "0.01.2", "0.1.2-rc.1", "0.1.2+abc", "0.1.2\n", "vgarbage"]:
            with self.assertRaises(ValueError, msg=value):
                parse_version(value)

    def test_real_cargo_files_only_change_the_cli_version(self):
        root = Path(__file__).resolve().parents[3]
        for filename, replace in [("Cargo.toml", replace_manifest_version), ("Cargo.lock", replace_lock_version)]:
            original = (root / filename).read_text()
            previous = re.search(r'name = "huckleberry-cli"\nversion = "([^"]+)"', original)[1]
            changed = replace(original, "0.999.999")
            self.assertNotEqual(changed, original)
            self.assertEqual(changed.replace('version = "0.999.999"', f'version = "{previous}"'), original)
            self.assertIn('version = "0.6.0"', changed)

    def test_missing_or_duplicate_package_is_rejected(self):
        manifest = '[package]\nname = "huckleberry-cli"\nversion = "0.47.2"\n'
        lock = '[[package]]\nname = "huckleberry-cli"\nversion = "0.47.2"\n'
        for source, replace in [(manifest, replace_manifest_version), (lock, replace_lock_version)]:
            for invalid in [source + source, source.replace("huckleberry-cli", "another")]:
                with self.assertRaises(ValueError):
                    replace(invalid, "0.48.0")
