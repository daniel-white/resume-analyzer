"""Per-run demo variation preserves parser biases and callback determinism."""

import unittest
from unittest.mock import patch

from providers import stub
from providers.test_smart_resume import fixture


class DemoTests(unittest.TestCase):
    def test_demo_delays_are_longer_and_can_be_overridden(self):
        with patch.dict(stub.os.environ, {"ATS_DELAY": ""}):
            for provider in stub.FLEET:
                low, high = stub.delay_band(provider)
                self.assertGreaterEqual(low, 10)
                self.assertLessEqual(high, 30)
                self.assertGreater(high, low)
        with patch.dict(stub.os.environ, {"ATS_DELAY": "0,0"}):
            self.assertEqual(stub.delay_band(stub.FLEET[0]), (0, 0))

    def test_retries_are_identical_and_new_runs_vary_for_every_provider(self):
        for designed in (False, True):
            parsed = stub.read_file(stub.DOCX, fixture(designed))
            for provider in stub.FLEET:
                with self.subTest(provider=provider.id, designed=designed):
                    results = [stub.demo_decide(provider.id, str(i), parsed) for i in range(40)]
                    self.assertEqual(results[0], stub.demo_decide(provider.id, "0", parsed))
                    self.assertGreater(len({str(r["factors"]) for r in results}), 1)
                    self.assertGreater(len({r["score"] for r in results}), 1)
                    baseline = stub.decide(provider.id, parsed)
                    for result in results:
                        self.assertEqual(result["kept"], baseline["kept"])
                        self.assertEqual(result["dropped"], baseline["dropped"])
                        for original, varied in zip(baseline["factors"], result["factors"]):
                            self.assertEqual(original["label"], varied["label"])
                            self.assertEqual(original["delta"] > 0, varied["delta"] > 0)
                            if original["delta"] < 0:
                                self.assertGreater(varied["delta"], original["delta"])
                        expected = max(0, min(100, stub.base_for(provider.id)
                                              + sum(f["delta"] for f in result["factors"])))
                        self.assertEqual(result["score"], expected)
                        self.assertEqual(result["summary"], stub.summarize(expected, result["factors"]))
                    if designed:
                        self.assertGreater(sum(r["score"] for r in results) / len(results), baseline["score"])

    def test_positive_findings_only_describe_supported_layout(self):
        parsed = stub.read_file(stub.DOCX, fixture(True))
        for provider in stub.FLEET:
            result = stub.demo_decide(provider.id, "demo", parsed)
            self.assertEqual(len(result["factors"]), len(stub.decide(provider.id, parsed)["factors"]))

    def test_unreadable_files_never_get_random_rewards(self):
        parsed = stub.read_file(stub.DOCX, b"not a zip")
        for provider in stub.FLEET:
            for run_id in ("first", "second"):
                self.assertEqual(stub.demo_decide(provider.id, run_id, parsed), stub.decide(provider.id, parsed))

    def test_reports_are_distinct_stable_and_grounded_in_the_read(self):
        parsed = stub.read_file(stub.DOCX, fixture(True))
        engines = set()
        for item in stub.FLEET:
            result = stub.delivered_verdict(item, "run", parsed)
            self.assertEqual(result, stub.delivered_verdict(item, "RUN", parsed))
            report = result["report"]
            engines.add(report["engine"])
            self.assertNotEqual(report["reference"], stub.delivered_verdict(item, "new", parsed)["report"]["reference"])
            self.assertEqual(report["contact_fields"], 2)
            self.assertEqual(report["sections_detected"], len(parsed.sections))
            self.assertEqual(report["bullets_detected"], parsed.bullet_count)
            self.assertEqual(report["layout_confidence"], "structural")
            self.assertEqual(report["outcome"], "parsed_with_warnings" if result["dropped"] else "parsed")
            self.assertEqual({k: v for k, v in result.items() if k != "report"}, stub.demo_decide(item.id, "run", parsed))
        self.assertEqual(len(engines), len(stub.FLEET))

    def test_failed_reports_and_pdf_limitations_are_explicit(self):
        for item in stub.FLEET:
            report = stub.delivered_verdict(item, "run", stub.read_file(stub.DOCX, b"bad"))["report"]
            self.assertEqual(report["outcome"], "unreadable")
            self.assertEqual((report["contact_fields"], report["sections_detected"], report["bullets_detected"]), (0, 0, 0))
            report = stub.delivered_verdict(item, "run", stub.read_file(stub.PDF, b"%PDF-1.4"))["report"]
            self.assertEqual(report["layout_confidence"], "inferred")
            self.assertTrue(any("limited to embedded text" in note for note in report["notes"]))


if __name__ == "__main__":
    unittest.main()
