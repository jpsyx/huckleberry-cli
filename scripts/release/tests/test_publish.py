"""Retry publication without duplicate tags, releases, or version rollback."""
import unittest
import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from unittest.mock import patch
from scripts.release.repository import Push, Repository
from scripts.release.publish import publish_push
from scripts.release.github import GitHub
from support import Fixture, FakeGitHub, git


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.fixture = Fixture()
        self.addCleanup(self.fixture.close)
        self.repo = Repository(self.fixture.path)
        self.github = FakeGitHub()
        self.push = Push(self.fixture.baseline, self.fixture.commit("feat: modal `text` $(literal)"), 1)

    def publish(self, push=None):
        return publish_push(self.repo, self.github, push or self.push, lambda record: None)

    def test_bootstrap_then_retry_creates_one_release(self):
        original = self.publish()
        self.assertEqual(original.version, "0.48.0")
        self.assertEqual(self.publish(), original)
        self.assertEqual(self.github.created_tags, ["v0.48.0"])
        self.assertIn("$(literal)", self.github.releases["v0.48.0"]["body"])
        self.assertIn(self.push.source, self.github.releases["v0.48.0"]["body"])

    def test_bootstrap_uses_the_source_version_when_main_advanced_before_activation(self):
        self.repo.write_version("0.48.1")
        source = self.fixture.commit("feat: activate automatic releases")
        record = self.publish(Push(self.push.source, source, 2))
        self.assertEqual(record.version, "0.49.0")
        self.repo.fetch()
        self.assertIn('version = "0.49.0"', self.repo.contents("origin/main", "Cargo.toml"))

    def test_retry_after_lost_create_response_reuses_release(self):
        self.github.fail_after_create = True
        with self.assertRaises(ConnectionError):
            self.publish()
        self.assertEqual(self.publish().version, "0.48.0")
        self.assertEqual(self.github.created_tags, ["v0.48.0"])

    def test_retry_after_tag_push_response_loss(self):
        original = self.repo.push_tag
        def lost(record):
            original(record)
            raise ConnectionError("tag push response lost")
        with patch.object(self.repo, "push_tag", side_effect=lost), self.assertRaises(ConnectionError):
            self.publish()
        self.assertEqual(self.publish().version, "0.48.0")
        self.assertEqual(self.github.created_tags, ["v0.48.0"])

    def test_retry_after_main_sync_failure(self):
        with patch.object(self.repo, "sync_main", side_effect=ConnectionError), self.assertRaises(ConnectionError):
            self.publish()
        self.assertEqual(self.publish().version, "0.48.0")
        self.assertEqual(self.github.created_tags, ["v0.48.0"])

    def test_failed_verification_publishes_nothing(self):
        with self.assertRaises(RuntimeError):
            publish_push(self.repo, self.github, self.push, lambda record: (_ for _ in ()).throw(RuntimeError("bad build")))
        self.assertEqual(git(self.fixture.origin, "tag", "--list"), "")
        self.assertEqual(self.github.created_tags, [])

    def test_reserved_older_retry_does_not_become_latest_or_lower_main(self):
        self.repo.fetch()
        older = self.repo.prepare(self.push, "0.48.0")
        self.repo.push_tag(older)
        git(self.fixture.path, "checkout", "main")
        newer = self.fixture.commit("fix: newer", "new.txt", "new")
        self.publish(Push(self.push.source, newer, 2))
        self.assertEqual(self.publish(), older)
        self.assertEqual(self.github.latest, "v0.48.1")
        self.repo.fetch()
        self.assertIn('version = "0.48.1"', git(self.fixture.path, "show", "origin/main:Cargo.toml"))

    def test_unreserved_overtaken_push_is_reported_without_stale_release(self):
        newer = self.fixture.commit("fix: newer", "new.txt", "new")
        self.publish(Push(self.push.source, newer, 2))
        self.assertIsNone(self.publish())
        self.assertEqual(self.github.created_tags, ["v0.47.3"])


class GitHubTransportTests(unittest.TestCase):
    def setUp(self):
        self.requests = []
        self.created = None
        self.lose_response = False
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_arguments):
                pass

            def do_GET(self):
                owner.requests.append((self.path, None, self.headers.get("Authorization")))
                self.send_response(200 if owner.created else 404)
                self.end_headers()
                self.wfile.write(json.dumps(owner.created or {}).encode())

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                owner.requests.append((self.path, body, self.headers.get("Authorization")))
                owner.created = body
                if owner.lose_response:
                    owner.lose_response = False
                    self.close_connection = True
                    return
                self.send_response(201)
                self.end_headers()
                self.wfile.write(json.dumps(body).encode())

        server = HTTPServer(("127.0.0.1", 0), Handler)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.client = GitHub("jpsyx/huckleberry-cli", "test-token", f"http://127.0.0.1:{server.server_port}")

    def test_creates_public_release_with_generated_notes(self):
        from scripts.release.repository import ReleaseRecord
        record = ReleaseRecord("0.48.0", "a" * 40, "b" * 40, "c" * 40, 1)
        self.assertIsNone(self.client.release("v0.48.0"))
        result = self.client.publish(record, "Direct commit notes", True)
        self.assertEqual(result["tag_name"], "v0.48.0")
        body = self.requests[-1][1]
        self.assertTrue(body["generate_release_notes"])
        self.assertEqual(body["body"], "Direct commit notes")
        self.assertEqual(body["make_latest"], "true")
        self.assertFalse(body["draft"] or body["prerelease"])
        self.assertEqual(self.requests[-1][2], "Bearer test-token")

    def test_lost_response_recovers_without_repeating_the_post(self):
        from scripts.release.repository import ReleaseRecord
        self.lose_response = True
        result = self.client.publish(ReleaseRecord("0.48.0", "a" * 40, "b" * 40, "c" * 40, 1), "Notes", False)
        self.assertEqual(result["tag_name"], "v0.48.0")
        self.assertEqual(len([request for request in self.requests if request[1]]), 1)
        self.assertEqual(result["make_latest"], "false")
