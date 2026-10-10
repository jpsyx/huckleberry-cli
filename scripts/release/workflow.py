"""Wait for preceding runs before entering the serialized publication job."""
import time
from typing import Sequence

from .github import GitHub


def pending_predecessors(runs: Sequence[dict], run_id: int, run_number: int) -> list[int]:
    """Select only unfinished older main push runs of the same workflow."""
    return sorted(run["id"] for run in runs
                  if run["id"] != run_id and run["run_number"] < run_number
                  and run["head_branch"] == "main" and run["event"] == "push"
                  and run["status"] != "completed")


def wait_for_predecessors(github: GitHub, workflow: str, run_id: int, run_number: int) -> None:
    """Bound waiting to 90 minutes; do not hold the publication lock here."""
    deadline = time.monotonic() + 90 * 60
    while pending_predecessors(github.runs(workflow), run_id, run_number):
        if time.monotonic() >= deadline:
            raise TimeoutError("older release runs did not finish within 90 minutes")
        time.sleep(15)
