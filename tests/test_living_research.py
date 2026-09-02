import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

from living_research import (  # noqa: E402
    CAPTURE_SCHEMA,
    REVIEW_RESPONSE_SCHEMA,
    LivingResearchLedger,
    LedgerError,
    adapt_csv_capture,
    canonical_csv,
    canonical_html,
    canonical_json,
    csv_registration,
    html_registration,
    iso_at,
    json_registration,
    packet,
    run_qualification,
)


class LivingResearchTests(unittest.TestCase):
    def test_html_cosmetic_markup_is_normalized(self):
        first = canonical_html("<p class='old'>Effective date is January 1.</p>")
        second = canonical_html("<p class='new'> Effective date is January 1. </p>")
        self.assertEqual(first, second)

    def test_json_cosmetic_pointer_is_ignored(self):
        first = canonical_json('{"threshold":5,"generated_at":"one"}', ["/generated_at"])
        second = canonical_json('{ "generated_at":"two", "threshold":5 }', ["/generated_at"])
        self.assertEqual(first, second)

    def test_json_empty_container_type_change_is_material_and_claim_bound(self):
        ledger = LivingResearchLedger()
        registration = json_registration()
        ledger.register_source(registration, iso_at(0))
        ledger.capture(
            packet(
                registration["source_id"],
                registration["source_kind"],
                1,
                "observed",
                registration["canonical_locator"],
                '{"threshold":{}}',
                "1",
            )
        )
        result = ledger.capture(
            packet(
                registration["source_id"],
                registration["source_kind"],
                2,
                "observed",
                registration["canonical_locator"],
                '{"threshold":[]}',
                "2",
            )
        )
        self.assertEqual(result.materiality, "potentially_material")
        self.assertEqual(result.affected_claim_ids, ["claim-json-threshold"])
        self.assertIn("/threshold", ledger.state["proposals"][-1]["changed_selectors"])

    def test_html_reordering_and_multiplicity_changes_are_not_cosmetic(self):
        changes = [
            (
                "<p>Effective date is January 1.</p><p>Threshold remains five.</p>",
                "<p>Threshold remains five.</p><p>Effective date is January 1.</p>",
            ),
            (
                "<p>Effective date is January 1.</p><p>Effective date is January 1.</p>",
                "<p>Effective date is January 1.</p>",
            ),
        ]
        for before, after in changes:
            with self.subTest(after=after):
                ledger = LivingResearchLedger()
                registration = html_registration()
                ledger.register_source(registration, iso_at(0))
                ledger.capture(
                    packet(
                        registration["source_id"],
                        registration["source_kind"],
                        1,
                        "observed",
                        registration["canonical_locator"],
                        before,
                        "1",
                    )
                )
                result = ledger.capture(
                    packet(
                        registration["source_id"],
                        registration["source_kind"],
                        2,
                        "observed",
                        registration["canonical_locator"],
                        after,
                        "2",
                    )
                )
                self.assertEqual(result.materiality, "potentially_material")
                self.assertEqual(
                    result.affected_claim_ids, ["claim-html-effective-date"]
                )

    def test_csv_row_order_line_endings_and_ignored_column_are_cosmetic(self):
        options = csv_registration()["tabular_csv"]
        first = canonical_csv(
            "region,threshold,status,generated_at\r\nnorth,5,active,one\r\nsouth,4,active,one\r\n",
            options,
        )
        second = canonical_csv(
            "region,threshold,status,generated_at\nsouth,4,active,two\nnorth,5,active,two\n",
            options,
        )
        self.assertEqual(first, second)

    def test_csv_rejects_duplicate_header_key_and_wrong_row_width(self):
        options = csv_registration()["tabular_csv"]
        invalid = [
            "region,threshold,threshold\nnorth,5,5\n",
            "region,threshold\nnorth,5\nnorth,7\n",
            "region,threshold\nnorth\n",
        ]
        for raw in invalid:
            with self.subTest(raw=raw):
                with self.assertRaises(LedgerError):
                    canonical_csv(raw, options)

    def test_csv_cell_change_maps_to_claim_without_truth_rewrite(self):
        ledger = LivingResearchLedger()
        registration = csv_registration()
        ledger.register_source(registration, iso_at(0))
        ledger.capture(
            packet(
                registration["source_id"],
                "tabular_csv",
                1,
                "observed",
                registration["canonical_locator"],
                "region,threshold,status,generated_at\nnorth,5,active,one\n",
                "1",
            )
        )
        before = ledger.current_conclusions()
        result = ledger.capture(
            packet(
                registration["source_id"],
                "tabular_csv",
                2,
                "observed",
                registration["canonical_locator"],
                "region,threshold,status,generated_at\nnorth,7,active,two\n",
                "2",
            )
        )
        proposal = ledger.state["proposals"][-1]
        self.assertEqual(result.materiality, "potentially_material")
        self.assertEqual(result.affected_claim_ids, ["claim-csv-north-threshold"])
        self.assertIn("/rows/north/threshold", proposal["changed_selectors"])
        self.assertEqual(ledger.current_conclusions(), before)

    def test_csv_adapter_converts_malformed_input_to_failure_packet(self):
        registration = csv_registration()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.csv"
            path.write_text("region,threshold\nnorth\n", encoding="utf-8")
            capture = adapt_csv_capture(
                registration,
                csv_path=path,
                observed_at=iso_at(1),
                declared_version="1",
            )
        self.assertEqual(capture["status"], "malformed")
        self.assertIsNone(capture["content"])
        self.assertIn("csv_adapter_validation_error", capture["extraction_warnings"])

    def test_independent_csv_reviews_reach_quorum_without_truth_rewrite(self):
        ledger, proposal_id = self._csv_proposal()
        before = ledger.current_conclusions()
        approved = {"claim-csv-north-threshold": "The North region threshold is 7."}
        first = ledger.record_review_response(
            self._review_response(proposal_id, "human-reviewer-a", "approved", approved)
        )
        second = ledger.record_review_response(
            self._review_response(proposal_id, "human-reviewer-b", "approved", approved)
        )
        self.assertEqual(first["review_state"], "awaiting_responses")
        self.assertEqual(second["review_state"], "quorum_reached")
        self.assertEqual(second["distinct_reviewer_count"], 2)
        self.assertEqual(ledger.current_conclusions(), before)
        self.assertEqual(ledger.state["supersessions"], [])

    def test_duplicate_or_conflicting_csv_reviews_do_not_revise_claim(self):
        ledger, proposal_id = self._csv_proposal()
        before = ledger.current_conclusions()
        approved = {"claim-csv-north-threshold": "The North region threshold is 7."}
        ledger.record_review_response(
            self._review_response(proposal_id, "human-reviewer-a", "approved", approved)
        )
        with self.assertRaises(LedgerError):
            ledger.record_review_response(
                self._review_response(proposal_id, "human-reviewer-a", "approved", approved)
            )
        summary = ledger.record_review_response(
            self._review_response(proposal_id, "human-reviewer-b", "rejected", None)
        )
        self.assertEqual(summary["review_state"], "conflict")
        self.assertEqual(ledger.current_conclusions(), before)

    def test_review_packet_and_response_validate_reviewer_inputs(self):
        ledger, proposal_id = self._csv_proposal()
        with self.assertRaises(LedgerError):
            ledger.prepare_review_packet(
                proposal_id, ["human-reviewer-a", "human-reviewer-a"]
            )
        with self.assertRaises(LedgerError):
            ledger.prepare_review_packet(proposal_id, ["human-reviewer-a", ""])
        invalid = self._review_response(
            proposal_id, "human-reviewer-a", "approved", {"claim-csv-north-threshold": ""}
        )
        with self.assertRaises(LedgerError):
            ledger.record_review_response(invalid)
        invalid_timestamp = self._review_response(
            proposal_id,
            "human-reviewer-a",
            "approved",
            {"claim-csv-north-threshold": "The North region threshold is 7."},
        )
        invalid_timestamp["reviewed_at"] = "not-a-timestamp"
        with self.assertRaises(LedgerError):
            ledger.record_review_response(invalid_timestamp)

    def _csv_proposal(self):
        ledger = LivingResearchLedger()
        registration = csv_registration()
        ledger.register_source(registration, iso_at(0))
        ledger.capture(
            packet(
                registration["source_id"],
                "tabular_csv",
                1,
                "observed",
                registration["canonical_locator"],
                "region,threshold,status,generated_at\nnorth,5,active,one\n",
                "1",
            )
        )
        result = ledger.capture(
            packet(
                registration["source_id"],
                "tabular_csv",
                2,
                "observed",
                registration["canonical_locator"],
                "region,threshold,status,generated_at\nnorth,7,active,two\n",
                "2",
            )
        )
        return ledger, result.proposal_id

    def _review_response(self, proposal_id, reviewer, outcome, approved):
        return {
            "schema": REVIEW_RESPONSE_SCHEMA,
            "proposal_id": proposal_id,
            "reviewer": reviewer,
            "reviewed_at": iso_at(10),
            "outcome": outcome,
            "rationale": "Independent fixture review.",
            "understandable": True,
            "materially_relevant": True,
            "approved_conclusions": approved,
        }

    def test_capture_does_not_change_accepted_conclusion(self):
        ledger = LivingResearchLedger()
        ledger.register_source(html_registration(), iso_at(0))
        ledger.capture(
            packet(
                "source-html-policy",
                "html_document",
                1,
                "observed",
                "https://standards.example.test/policy",
                "<p>Effective date is January 1.</p>",
                "1",
            )
        )
        before = ledger.current_conclusions()
        result = ledger.capture(
            packet(
                "source-html-policy",
                "html_document",
                2,
                "observed",
                "https://standards.example.test/policy",
                "<p>Effective date is February 1.</p>",
                "2",
            )
        )
        self.assertEqual(result.materiality, "potentially_material")
        self.assertIsNotNone(result.proposal_id)
        self.assertFalse(result.accepted_conclusions_changed)
        self.assertEqual(ledger.current_conclusions(), before)

    def test_explicit_review_creates_supersession(self):
        ledger = LivingResearchLedger()
        ledger.register_source(json_registration(), iso_at(0))
        ledger.capture(
            packet(
                "source-json-standard",
                "structured_json",
                1,
                "observed",
                "https://data.example.test/standard.json",
                '{"threshold":5}',
                "1",
            )
        )
        result = ledger.capture(
            packet(
                "source-json-standard",
                "structured_json",
                2,
                "observed",
                "https://data.example.test/standard.json",
                '{"threshold":7}',
                "2",
            )
        )
        ledger.review(
            {
                "proposal_id": result.proposal_id,
                "reviewer": "test-reviewer",
                "reviewed_at": iso_at(3),
                "rationale": "Verified structured source update.",
                "approved_conclusions": {"claim-json-threshold": "Threshold is 7."},
            }
        )
        self.assertEqual(ledger.current_conclusions()["claim-json-threshold"], "Threshold is 7.")
        self.assertEqual(len(ledger.state["claim_history"]["claim-json-threshold"]), 2)
        self.assertEqual(len(ledger.state["supersessions"]), 1)

    def test_direct_review_rejects_malformed_metadata_without_mutation(self):
        invalid_overrides = [
            {"reviewer": ""},
            {"reviewed_at": "not-a-timestamp"},
            {"rationale": ""},
            {"approved_conclusions": {"claim-json-threshold": 7}},
            {"approved_conclusions": {"": "Threshold is 7."}},
        ]
        for override in invalid_overrides:
            with self.subTest(override=override):
                ledger = LivingResearchLedger()
                registration = json_registration()
                ledger.register_source(registration, iso_at(0))
                ledger.capture(
                    packet(
                        registration["source_id"],
                        registration["source_kind"],
                        1,
                        "observed",
                        registration["canonical_locator"],
                        '{"threshold":5}',
                        "1",
                    )
                )
                result = ledger.capture(
                    packet(
                        registration["source_id"],
                        registration["source_kind"],
                        2,
                        "observed",
                        registration["canonical_locator"],
                        '{"threshold":7}',
                        "2",
                    )
                )
                decision = {
                    "proposal_id": result.proposal_id,
                    "reviewer": "test-reviewer",
                    "reviewed_at": iso_at(3),
                    "rationale": "Verified structured source update.",
                    "approved_conclusions": {
                        "claim-json-threshold": "Threshold is 7."
                    },
                    **override,
                }
                before = json.dumps(ledger.state, sort_keys=True)
                with self.assertRaises(LedgerError):
                    ledger.review(decision)
                self.assertEqual(json.dumps(ledger.state, sort_keys=True), before)

    def test_capture_locator_must_match_canonical_or_declared_alias(self):
        ledger = LivingResearchLedger()
        registration = html_registration()
        alias = "https://standards.example.test/policy-v2"
        registration["locator_aliases"] = [alias]
        ledger.register_source(registration, iso_at(0))
        ledger.capture(
            packet(
                registration["source_id"],
                registration["source_kind"],
                1,
                "observed",
                registration["canonical_locator"],
                "<p>Effective date is January 1.</p>",
                "1",
            )
        )
        before = json.dumps(ledger.state, sort_keys=True)
        with self.assertRaises(LedgerError):
            ledger.capture(
                packet(
                    registration["source_id"],
                    registration["source_kind"],
                    2,
                    "relocated",
                    "https://unknown.example.test/policy",
                    "<p>Effective date is January 1.</p>",
                    "1",
                )
            )
        self.assertEqual(json.dumps(ledger.state, sort_keys=True), before)
        accepted = ledger.capture(
            packet(
                registration["source_id"],
                registration["source_kind"],
                2,
                "relocated",
                alias,
                "<p>Effective date is January 1.</p>",
                "1",
            )
        )
        self.assertEqual(accepted.materiality, "cosmetic")

    def test_capture_rejects_malformed_observation_time_without_mutation(self):
        for observed_at in [None, "not-a-timestamp", "2026-08-23T01:00:00"]:
            with self.subTest(observed_at=observed_at):
                ledger = LivingResearchLedger()
                registration = html_registration()
                ledger.register_source(registration, iso_at(0))
                capture = packet(
                    registration["source_id"],
                    registration["source_kind"],
                    1,
                    "observed",
                    registration["canonical_locator"],
                    "<p>Effective date is January 1.</p>",
                    "1",
                )
                capture["observed_at"] = observed_at
                before = json.dumps(ledger.state, sort_keys=True)
                with self.assertRaises(LedgerError):
                    ledger.capture(capture)
                self.assertEqual(json.dumps(ledger.state, sort_keys=True), before)

    def test_concurrent_cli_ingests_preserve_both_versions(self):
        root = Path(__file__).resolve().parents[1]
        tool = root / "tools" / "living_research.py"
        registry = root / "examples" / "living-research" / "source-registry.json"
        packets = [
            root / "examples" / "living-research" / "pagediff-baseline.json",
            root / "examples" / "living-research" / "pagediff-material-change.json",
        ]
        with tempfile.TemporaryDirectory() as directory:
            ledger_path = Path(directory) / "ledger.json"
            initialized = subprocess.run(
                [
                    sys.executable,
                    str(tool),
                    "init",
                    "--registry",
                    str(registry),
                    "--ledger",
                    str(ledger_path),
                ],
                cwd=root,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(initialized.returncode, 0, initialized.stderr)
            processes = [
                subprocess.Popen(
                    [
                        sys.executable,
                        str(tool),
                        "ingest",
                        "--ledger",
                        str(ledger_path),
                        "--packet",
                        str(packet_path),
                    ],
                    cwd=root,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                for packet_path in packets
            ]
            outcomes = [process.communicate(timeout=30) for process in processes]
            self.assertEqual(
                [process.returncode for process in processes],
                [0, 0],
                outcomes,
            )
            state = json.loads(ledger_path.read_text(encoding="utf-8"))
            self.assertEqual(len(state["versions"]["web-source-test-policy"]), 2)

    def test_deferred_review_preserves_conclusion_and_remains_reviewable(self):
        ledger = LivingResearchLedger()
        ledger.register_source(html_registration(), iso_at(0))
        ledger.capture(
            packet(
                "source-html-policy",
                "html_document",
                1,
                "observed",
                "https://standards.example.test/policy",
                "<p>Effective date is January 1.</p>",
                "1",
            )
        )
        result = ledger.capture(
            packet(
                "source-html-policy",
                "html_document",
                2,
                "observed",
                "https://standards.example.test/policy",
                "<p>Effective date is February 1.</p>",
                "2",
            )
        )
        before = ledger.current_conclusions()
        ledger.review(
            {
                "proposal_id": result.proposal_id,
                "outcome": "deferred",
                "reviewer": "pilot-reviewer",
                "reviewed_at": iso_at(3),
                "rationale": "Await corroborating evidence.",
            }
        )
        self.assertEqual(ledger.current_conclusions(), before)
        self.assertEqual(ledger.state["proposals"][-1]["status"], "deferred")
        self.assertEqual(len(ledger.pending_proposals()), 1)
        self.assertEqual(ledger.state["supersessions"], [])

    def test_page_diff_packet_shape_is_consumed(self):
        ledger = LivingResearchLedger()
        source = html_registration()
        source["source_id"] = "web-source-abcd"
        ledger.register_source(source, iso_at(0))
        capture = {
            "schema": CAPTURE_SCHEMA,
            "source_id": "web-source-abcd",
            "source_kind": "html_document",
            "locator": "https://standards.example.test/policy",
            "observed_at": iso_at(1),
            "status": "observed",
            "content": "<p>Effective date is January 1.</p>",
            "declared_version": None,
            "response_status": 200,
            "retry_after_seconds": None,
            "extraction_warnings": [],
            "extraction_method": "readability_service_worker",
            "content_identity": "sha256:adapter-owned-advisory",
            "source_metadata": {"trust": "authoritative"},
            "page_diff": {"added_sentences": 1, "removed_sentences": 0},
        }
        result = ledger.capture(capture)
        self.assertEqual(result.materiality, "baseline")
        self.assertEqual(len(ledger.state["versions"]["web-source-abcd"]), 1)

    def test_ledger_round_trip_preserves_history(self):
        ledger = LivingResearchLedger()
        ledger.register_source(json_registration(), iso_at(0))
        ledger.capture(
            packet(
                "source-json-standard",
                "structured_json",
                1,
                "observed",
                "https://data.example.test/standard.json",
                '{"threshold":5}',
                "1",
            )
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ledger.json"
            ledger.save(path)
            restored = LivingResearchLedger.load(path)
        self.assertEqual(restored.state, ledger.state)

    def test_full_manual_qualification_matrix(self):
        bundle = run_qualification()
        report = bundle["report"]
        self.assertTrue(report["all_scenarios_passed"])
        self.assertEqual(report["metrics"]["missed_material_changes"], 0)
        self.assertEqual(report["metrics"]["false_positive_alerts"], 0)
        self.assertFalse(report["accepted_conclusions_changed_without_review"])
        self.assertEqual(report["approved_review_count"], 1)
        self.assertEqual(report["scheduler_state"], "unarmed")
        self.assertEqual(report["provider_reliability"], "UNKNOWN_NOT_EXERCISED")
        self.assertEqual(report["natural_recurrence"], "UNKNOWN_NOT_AUTHORIZED")
        names = {scenario["name"] for scenario in report["scenarios"]}
        self.assertTrue(
            {
                "html_material_edit",
                "source_deletion",
                "source_retraction",
                "source_inaccessible",
                "changed_url_same_content",
                "source_rate_limit",
                "malformed_content",
                "conflicting_same_version_update",
                "source_restoration",
            }.issubset(names)
        )

    def test_application_scheduler_is_source_gated_and_defaults_off(self):
        root = Path(__file__).resolve().parents[1]
        config = (root / "src" / "config.rs").read_text(encoding="utf-8")
        main = (root / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn("background_automation_enabled: bool", config)
        self.assertIn('std::env::var("BACKGROUND_AUTOMATION_ENABLED")', config)
        self.assertIn("parse_background_automation_enabled(None)", config)
        self.assertIn("if config.background_automation_enabled", main)
        self.assertIn("Background automation is unarmed", main)


if __name__ == "__main__":
    unittest.main()
