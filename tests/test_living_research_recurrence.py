import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_recurrence_module():
    import importlib.util

    path = ROOT / "tools" / "living_research_recurrence.py"
    spec = importlib.util.spec_from_file_location("living_research_recurrence", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class LivingResearchRecurrenceTests(unittest.TestCase):
    def test_two_timer_firings_write_distinct_terminal_receipts(self):
        recurrence = load_recurrence_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ledger = root / "ledger.json"
            receipts = root / "receipts"
            summary = recurrence.run_recurrence(
                repo_root=ROOT,
                registry=ROOT / "examples/living-research/source-registry.json",
                ledger=ledger,
                packets=[
                    ROOT / "examples/living-research/pagediff-baseline.json",
                    ROOT / "examples/living-research/pagediff-material-change.json",
                ],
                receipt_dir=receipts,
                interval_seconds=0.02,
                firing_count=2,
            )

            self.assertTrue(summary["all_receipts_passed"])
            self.assertEqual(summary["observed_firings"], 2)
            self.assertEqual(summary["distinct_timer_receipts"], 2)
            self.assertEqual(len(list(receipts.glob("receipt-*.json"))), 2)
            ledger_state = json.loads(ledger.read_text(encoding="utf-8"))
            self.assertEqual(ledger_state["scheduler_state"], "unarmed")
            self.assertEqual(len(ledger_state["versions"]["web-source-test-policy"]), 2)


if __name__ == "__main__":
    unittest.main()
