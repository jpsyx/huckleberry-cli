"""Release runs queue before allocating runners, retaining pending pushes."""
from pathlib import Path
import re
import unittest


class WorkflowTests(unittest.TestCase):
    def test_waiting_release_runs_do_not_occupy_runners(self):
        workflow = (Path(__file__).resolve().parents[3] / '.github/workflows/release.yml').read_text()
        concurrency = re.search(r'(?m)^concurrency:\n((?:[ \t].*\n)+)', workflow)
        self.assertIsNotNone(concurrency, 'queue the whole workflow before jobs allocate runners')
        self.assertRegex(concurrency[1], r'(?m)^  queue: max$')
        self.assertNotIn('cancel-in-progress: true', workflow)
        self.assertNotRegex(workflow, r'(?m)^    concurrency:')
        self.assertNotIn('-m scripts.release wait', workflow)

    def test_all_main_pushes_must_pass_validation_before_publication(self):
        workflow = (Path(__file__).resolve().parents[3] / '.github/workflows/release.yml').read_text()
        self.assertIn('branches: [main]', workflow)
        self.assertNotRegex(workflow, r'(?m)^\s+paths(?:-ignore)?:')
        self.assertRegex(workflow, r'publish:\n\s+needs: validate')
        for check in ['cargo fmt --check', 'cargo clippy', 'cargo test --locked', 'unittest discover']:
            self.assertIn(check, workflow)
