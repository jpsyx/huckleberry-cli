"""Small GitHub REST boundary with bounded retries and ambiguous-write recovery."""
import http.client
import json
import re
import time
from typing import Optional
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

from .repository import ReleaseRecord


class RemoteError(RuntimeError):
    """An HTTP failure whose status determines whether retrying can help."""
    def __init__(self, status: int):
        super().__init__(f"GitHub request failed (HTTP {status})")
        self.status = status
        self.retryable = status == 429 or status >= 500


class GitHub:
    """Authenticated release transport; tokens are never included in diagnostics."""
    def __init__(self, repository: str, token: str, base_url: str = "https://api.github.com"):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("invalid GitHub repository name")
        self.repository = repository
        self.base_url = base_url.rstrip("/") + "/repos/" + repository
        self.token = token

    def request(self, method: str, path: str, payload: Optional[dict] = None) -> dict:
        data = None if payload is None else json.dumps(payload).encode()
        request = Request(self.base_url + path, data=data, method=method, headers={
            "Authorization": "Bearer " + self.token,
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "User-Agent": "huckleberry-release-workflow",
            "X-GitHub-Api-Version": "2026-03-10",
        })
        try:
            with urlopen(request, timeout=30) as response:
                return json.load(response)
        except HTTPError as error:
            raise RemoteError(error.code) from None

    def read(self, path: str, missing_ok: bool = False) -> Optional[dict]:
        for attempt in range(5):
            try:
                return self.request("GET", path)
            except RemoteError as error:
                if error.status == 404 and missing_ok:
                    return None
                if not error.retryable or attempt == 4:
                    raise
            except (OSError, URLError, http.client.HTTPException):
                if attempt == 4:
                    raise RuntimeError("GitHub could not be reached") from None
            time.sleep(min(2 ** attempt, 30))
        raise RuntimeError("GitHub retry limit reached")

    def release(self, tag: str) -> Optional[dict]:
        return self.read("/releases/tags/" + quote(tag, safe=""), missing_ok=True)

    def publish(self, record: ReleaseRecord, body: str, make_latest: bool) -> dict:
        tag = "v" + record.version
        payload = {"tag_name": tag, "name": tag, "body": body,
                   "draft": False, "prerelease": False, "generate_release_notes": True,
                   "make_latest": "true" if make_latest else "false"}
        for attempt in range(5):
            try:
                return self.request("POST", "/releases", payload)
            except (RemoteError, OSError, URLError, http.client.HTTPException) as error:
                existing = self.release(tag)
                if existing is not None:
                    return existing
                if isinstance(error, RemoteError) and not error.retryable:
                    raise
                if attempt == 4:
                    raise RuntimeError("GitHub release could not be published") from None
                time.sleep(min(2 ** attempt, 30))
        raise RuntimeError("GitHub retry limit reached")

    def runs(self, workflow: str) -> list[dict]:
        runs = []
        page = 1
        while True:
            path = f"/actions/workflows/{quote(workflow, safe='')}/runs?branch=main&event=push&per_page=100&page={page}"
            batch = self.read(path)["workflow_runs"]
            runs.extend(batch)
            if len(batch) < 100:
                return runs
            page += 1
