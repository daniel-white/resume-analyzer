"""Raw-file scoring and private signed delivery for SmartResume."""

import base64
import hashlib
import hmac
import io
import json
import os
import threading
import time
import unittest
import urllib.request
import zipfile
from dataclasses import replace
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

from providers import stub


def fixture(designed):
    header = "<w:t>Ada Lovelace</w:t><w:t>ada@example.com</w:t>" if designed else ""
    body = '<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Experience</w:t></w:r></w:p>'
    if designed:
        body += '<w:cols w:num="2"/><w:tbl><w:t>Rust</w:t></w:tbl><w:txbxContent><w:t>sidebar</w:t></w:txbxContent><w:drawing/><w:t>2019</w:t>'
    else:
        body += '<w:t>Ada Lovelace</w:t><w:t>ada@example.com</w:t><w:t>01/2020</w:t><w:numPr/><w:numPr/>'
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w") as archive:
        archive.writestr("word/header1.xml", header)
        archive.writestr("word/document.xml", body)
    return data.getvalue()


class SmartResumeTests(unittest.TestCase):
    def test_designed_and_plain_factors(self):
        designed = stub.read_file(stub.DOCX, fixture(True))
        result = stub.decide("smart_resume", designed)
        self.assertEqual(
            [(f["label"], 1 if f["delta"] > 0 else -1) for f in result["factors"]],
            [("email kept", 1), ("table text kept", 1), ("text box", -1), ("standard headings", 1)],
        )
        self.assertEqual(result["dropped"], ["text box"])
        self.assertIn("table text", result["kept"])
        for item in stub.FLEET:
            if item.id != "smart_resume":
                self.assertNotEqual(
                    {f["label"] for f in result["factors"]},
                    {f["label"] for f in stub.decide(item.id, designed)["factors"]},
                )
        plain = stub.decide("smart_resume", stub.read_file(stub.DOCX, fixture(False)))
        self.assertEqual(plain["dropped"], [])
        self.assertTrue(all(f["delta"] > 0 for f in plain["factors"]))
        self.assertEqual([f["label"] for f in plain["factors"]],
                         ["email kept", "standard headings", "bullet density"])

    def test_inferred_layout_and_unreadable_file(self):
        parsed = replace(stub.empty_read("inferred", False), has_tables=True, has_text_boxes=True)
        result = stub.decide("smart_resume", parsed)
        self.assertEqual([f["label"] for f in result["factors"]],
                         ["table text kept (inferred)", "text box (inferred)"])
        unreadable = stub.decide("smart_resume", stub.read_file(stub.DOCX, b"not a zip"))
        self.assertEqual(unreadable["dropped"], ["file"])
        self.assertEqual(unreadable["factors"][0]["label"], "could not read file")
        self.assertLess(unreadable["factors"][0]["delta"], 0)

    def test_job_validation(self):
        item = stub.provider("smart_resume")
        payload = {
            "run_id": "00000000-0000-0000-0000-000000000123",
            "file": {"media_type": stub.DOCX, "bytes_base64": base64.b64encode(fixture(True)).decode()},
            "callback_url": "http://127.0.0.1:9999/api/callbacks/smart_resume",
        }
        self.assertEqual(stub.parse_job(item, payload).file, fixture(True))
        with self.assertRaisesRegex(ValueError, "webhook"):
            stub.parse_job(item, {**payload, "callback_url": "http://127.0.0.1:9999/api/callbacks/ash"})
        with self.assertRaisesRegex(ValueError, "5MB"):
            stub.parse_job(item, {**payload, "file": {
                "media_type": stub.DOCX,
                "bytes_base64": base64.b64encode(b"x" * (stub.MAX_FILE_BYTES + 1)).decode(),
            }})

    def test_health_raw_handoff_signature_retry_and_replay(self):
        item = stub.provider("smart_resume")
        attempts = []
        recorded = {}
        delivered = threading.Event()
        key = "smart-resume-test-secret"

        class Receiver(BaseHTTPRequestHandler):
            def do_POST(self):
                raw = self.rfile.read(int(self.headers["content-length"]))
                timestamp = int(self.headers["x-timestamp"])
                expected = hmac.new(key.encode(), f"{timestamp}.".encode() + raw, hashlib.sha256).hexdigest()
                valid = (self.path == "/api/callbacks/smart_resume"
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
                    self.assertEqual(json.load(response), {"id": "smart_resume"})
                payload = {
                    "run_id": "00000000-0000-0000-0000-000000000123",
                    "file": {"media_type": stub.DOCX, "bytes_base64": base64.b64encode(fixture(True)).decode()},
                    "callback_url": f"http://127.0.0.1:{receiver.server_port}/api/callbacks/smart_resume",
                }
                request = urllib.request.Request(f"http://127.0.0.1:{item.port}/",
                                                 data=json.dumps(payload).encode(),
                                                 headers={"content-type": "application/json"})
                with urllib.request.urlopen(request) as response:
                    self.assertEqual(response.status, 202)
                self.assertTrue(delivered.wait(5), "signed callback did not arrive")
                # Deliver the same run again: receiver records only the first key.
                job = stub.parse_job(item, payload)
                stub.deliver(item, job, stub.read_file(job.media_type, job.file))
                self.assertEqual(len(attempts), 3)
                self.assertTrue(all(attempt[2] for attempt in attempts))
                self.assertEqual(len(recorded), 1)
                self.assertTrue(all(attempt[0] == attempts[0][0] for attempt in attempts))
                self.assertTrue(all(attempt[1] == job.run_id for attempt in attempts))
                body = recorded[job.run_id]
                self.assertEqual(set(body), {"run_id", "parse", "verdict"})
                self.assertEqual(body["parse"], stub.as_parse(stub.read_file(stub.DOCX, job.file)))
                self.assertEqual(body["verdict"]["dropped"], ["text box"])
        finally:
            for server in servers:
                server.shutdown()
                server.server_close()
            for thread in threads:
                thread.join()


if __name__ == "__main__":
    unittest.main()
