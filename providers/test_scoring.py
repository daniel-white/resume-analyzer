"""Calibration checks; provider-specific bias tests remain in the fleet suite."""

import unittest

from providers import stub
from providers.test_smart_resume import fixture


class ScoringTests(unittest.TestCase):
    def test_readable_baseline_is_more_optimistic(self):
        parsed = stub.empty_read("inferred", False)
        for provider in stub.FLEET:
            with self.subTest(provider=provider.id):
                result = stub.decide(provider.id, parsed)
                old_base = 55 if provider.id == "ash" else 50
                expected = old_base + sum(f["delta"] for f in result["factors"]) + 20
                self.assertEqual(result["score"], expected)
                self.assertTrue(result["summary"].startswith(f"Score {expected} of 100."))

    def test_unreadable_files_do_not_receive_the_bonus(self):
        parsed = stub.read_file(stub.DOCX, b"not a zip")
        for provider in stub.FLEET:
            with self.subTest(provider=provider.id):
                result = stub.decide(provider.id, parsed)
                self.assertEqual(result["score"], 15 if provider.id == "ash" else 10)
                self.assertEqual(result["dropped"], ["file"])

    def test_plain_fixture_outscores_designed_and_scores_stay_bounded(self):
        plain = stub.read_file(stub.DOCX, fixture(False))
        designed = stub.read_file(stub.DOCX, fixture(True))
        for provider in stub.FLEET:
            with self.subTest(provider=provider.id):
                good = stub.decide(provider.id, plain)
                bad = stub.decide(provider.id, designed)
                self.assertGreaterEqual(good["score"], 85)
                self.assertGreater(good["score"], bad["score"])
                self.assertLessEqual(good["score"], 100)
                self.assertGreaterEqual(bad["score"], 0)
        self.assertEqual(stub.verdict([], [], [stub.factor("reward", 50, "")])["score"], 100)


if __name__ == "__main__":
    unittest.main()
