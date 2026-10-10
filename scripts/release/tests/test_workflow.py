"""Ordering and bounded waiting without holding the publication lock."""
import unittest
from unittest.mock import Mock, patch
from scripts.release.github import GitHub
from scripts.release.workflow import pending_predecessors, wait_for_predecessors


def run(number, status="in_progress", branch="main", event="push"):
    return {"id": number, "run_number": number, "status": status, "head_branch": branch, "event": event}


class WorkflowTests(unittest.TestCase):
    def test_only_older_unfinished_main_pushes_block(self):
        runs = [run(18, "completed"), run(19), run(20), run(21),
                run(16, branch="feature"), run(17, event="workflow_dispatch")]
        self.assertEqual(pending_predecessors(runs, 20, 20), [19])
        for status in ["queued", "waiting", "in_progress", "pending", "requested"]:
            self.assertEqual(pending_predecessors([run(19, status)], 20, 20), [19])
        self.assertEqual(pending_predecessors(runs, 18, 18), [])

    def test_waiting_ends_when_predecessor_completes(self):
        github = Mock()
        github.runs.side_effect = [[run(19)], [run(19, "completed")]]
        with patch("scripts.release.workflow.time.sleep") as sleep:
            wait_for_predecessors(github, "release.yml", 20, 20)
        sleep.assert_called_once_with(15)

    def test_wait_has_a_deadline(self):
        github = Mock()
        github.runs.return_value = [run(19)]
        with patch("scripts.release.workflow.time.monotonic", side_effect=[0, 5401]), self.assertRaises(TimeoutError):
            wait_for_predecessors(github, "release.yml", 20, 20)

    def test_github_runs_reads_every_page(self):
        github = GitHub("jpsyx/huckleberry-cli", "test-token")
        pages = [{"workflow_runs": [run(number) for number in range(100)]}, {"workflow_runs": [run(100)]}]
        with patch.object(github, "read", side_effect=pages):
            result = github.runs("release.yml")
        self.assertEqual([item["id"] for item in result], list(range(101)))
