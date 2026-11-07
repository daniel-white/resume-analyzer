"""Distinct compact-intake bias and signed private HTTP delivery."""

import base64
import hashlib
import hmac
import json
import os
import threading
import time
import unittest
import urllib.request
from dataclasses import replace
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

from providers import stub
from providers.test_smart_resume import fixture


class IndyHackersHiringTests(unittest.TestCase):
    def test_designed_and_plain(self):
        parsed = stub.read_file(stub.DOCX, fixture(True))
        result = stub.decide("indy_hackers_hiring", parsed)
        self.assertEqual([(f["label"], 1 if f["delta"] > 0 else -1) for f in result["factors"]],
                         [("email kept", 1), ("two columns", -1), ("image", -1), ("standard headings", 1)])
        self.assertEqual(result["dropped"], ["columns", "image"])
        for item in stub.FLEET:
            if item.id != "indy_hackers_hiring":
                self.assertNotEqual({f["label"] for f in result["factors"]},
                                    {f["label"] for f in stub.decide(item.id, parsed)["factors"]})
        plain = stub.decide("indy_hackers_hiring", stub.read_file(stub.DOCX, fixture(False)))
        self.assertEqual(plain["dropped"], [])
        self.assertTrue(all(f["delta"] > 0 for f in plain["factors"]))
        inferred = stub.decide("indy_hackers_hiring", replace(stub.empty_read("inferred", False), columns=2, has_tables=True))
        self.assertEqual(inferred["factors"][0]["label"], "two columns (inferred)")
        self.assertEqual(inferred["dropped"], ["columns"])

    def test_health_handoff_signature_retry_and_replay(self):
        item = stub.provider("indy_hackers_hiring")
        attempts, recorded = [], {}
        delivered = threading.Event()
        key = "indy-test-secret"

        class Receiver(BaseHTTPRequestHandler):
            def do_POST(self):
                raw = self.rfile.read(int(self.headers["content-length"]))
                timestamp = int(self.headers["x-timestamp"])
                expected = hmac.new(key.encode(), f"{timestamp}.".encode() + raw, hashlib.sha256).hexdigest()
                valid = (self.path == f"/api/callbacks/{item.id}"
                         and abs(time.time() - timestamp) <= 300
                         and hmac.compare_digest(expected, self.headers["x-signature"]))
                attempts.append((raw, self.headers["idempotency-key"], valid))
                status = 503 if len(attempts) == 1 else 200
                if status == 200:
                    recorded.setdefault(self.headers["idempotency-key"], json.loads(raw))
                self.send_response(status)
                self.send_header("content-length", "0")
                self.end_headers()
                if status == 200:
                    delivered.set()

            def log_message(self, *_):
                pass

        receiver = ThreadingHTTPServer(("127.0.0.1", 0), Receiver)
        provider = stub.ProviderServer(("127.0.0.1", item.port), stub.handler_for(item))
        servers = [receiver, provider]
        threads = [threading.Thread(target=s.serve_forever, daemon=True) for s in servers]
        for thread in threads:
            thread.start()
        try:
            with patch.dict(os.environ, {"PLUGIN_SECRET": key, "ATS_DELAY": "0,0"}):
                with urllib.request.urlopen(f"http://127.0.0.1:{item.port}/health") as response:
                    self.assertEqual(json.load(response), {"id": item.id})
                payload = {
                    "run_id": "00000000-0000-0000-0000-000000000123",
                    "file": {"media_type": stub.DOCX, "bytes_base64": base64.b64encode(fixture(True)).decode()},
                    "callback_url": f"http://127.0.0.1:{receiver.server_port}/api/callbacks/{item.id}",
                }
                with self.assertRaisesRegex(ValueError, "webhook"):
                    stub.parse_job(item, {**payload, "callback_url": "http://127.0.0.1:9999/api/callbacks/ash"})
                with self.assertRaisesRegex(ValueError, "5MB"):
                    stub.parse_job(item, {**payload, "file": {
                        "media_type": stub.DOCX,
                        "bytes_base64": base64.b64encode(b"x" * (stub.MAX_FILE_BYTES + 1)).decode(),
                    }})
                request = urllib.request.Request(f"http://127.0.0.1:{item.port}/", data=json.dumps(payload).encode(),
                                                 headers={"content-type": "application/json"})
                with urllib.request.urlopen(request) as response:
                    self.assertEqual(response.status, 202)
                self.assertTrue(delivered.wait(5), "signed callback did not arrive")
                job = stub.parse_job(item, payload)
                parsed = stub.read_file(job.media_type, job.file)
                stub.deliver(item, job, parsed)
                self.assertEqual(len(attempts), 3)
                self.assertTrue(all(a[2] for a in attempts))
                self.assertTrue(all(a[0] == attempts[0][0] and a[1] == job.run_id for a in attempts))
                self.assertEqual(len(recorded), 1)
                body = recorded[job.run_id]
                self.assertEqual(set(body), {"run_id", "parse", "verdict"})
                self.assertEqual(body["parse"], stub.as_parse(parsed))
                self.assertEqual(body["verdict"], stub.delivered_verdict(item, job.run_id, parsed))
                self.assertEqual(body["verdict"]["dropped"], ["columns", "image"])
        finally:
            for server in servers:
                server.shutdown()
                server.server_close()
            for thread in threads:
                thread.join()
