#!/usr/bin/env python3
"""Manual, review-gated research change ledger for KBFreshnessDetector.

The tool deliberately has no scheduler, provider client, notification path, or
automatic conclusion rewrite. It accepts portable PageDiff capture packets,
structured JSON captures, and explicitly keyed CSV datasets, then persists a
local append-only JSON ledger.
"""

from __future__ import annotations

import argparse
import copy
import csv
import difflib
import fcntl
import hashlib
import html
import io
import json
import os
import statistics
import sys
import time
import uuid
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from html.parser import HTMLParser
from pathlib import Path
from typing import Any, Iterable

CAPTURE_SCHEMA = "ResearchCapturePacketV1"
LEDGER_SCHEMA = "LivingResearchLedgerV1"
PROPOSAL_SCHEMA = "ResearchUpdateProposalV1"
REVIEW_RESPONSE_SCHEMA = "ResearchReviewerResponseV1"
QUALIFICATION_SCHEMA = "LivingResearchQualificationBundleV1"
REVIEW_REQUIRED = {"operational", "potentially_material", "critical"}
MATERIAL = {"potentially_material", "critical"}
CONTENT_STATUSES = {"observed", "relocated", "restored"}


class LedgerError(ValueError):
    pass


def stable_id(prefix: str, value: str) -> str:
    digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:24]
    return f"{prefix}-{digest}"


def iso_at(hour: int) -> str:
    value = datetime(2026, 8, 23, tzinfo=timezone.utc) + timedelta(hours=hour)
    return value.isoformat().replace("+00:00", "Z")


def require_text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise LedgerError(f"{field} must be a non-empty string")
    return value


def require_iso_timestamp(value: Any, field: str) -> str:
    text = require_text(value, field)
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError as error:
        raise LedgerError(f"{field} must be an ISO-8601 timestamp") from error
    if parsed.tzinfo is None:
        raise LedgerError(f"{field} must include a timezone")
    return text


def validate_approved_conclusions(value: Any) -> dict[str, str]:
    if not isinstance(value, dict) or not value:
        raise LedgerError("approved_conclusions must be a non-empty mapping")
    if any(
        not isinstance(claim_id, str)
        or not claim_id.strip()
        or not isinstance(conclusion, str)
        or not conclusion.strip()
        for claim_id, conclusion in value.items()
    ):
        raise LedgerError(
            "approved_conclusions must map non-empty claim IDs to non-empty strings"
        )
    return value


@contextmanager
def ledger_write_lock(path: Path):
    lock_path = path.with_suffix(path.suffix + ".lock")
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+", encoding="utf-8") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


class _VisibleText(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._ignored_depth = 0
        self.parts: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag.lower() in {"script", "style", "template"}:
            self._ignored_depth += 1
        elif not self._ignored_depth and tag.lower() in {
            "p",
            "div",
            "section",
            "article",
            "li",
            "h1",
            "h2",
            "h3",
            "br",
        }:
            self.parts.append(" ")

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() in {"script", "style", "template"} and self._ignored_depth:
            self._ignored_depth -= 1
        elif not self._ignored_depth:
            self.parts.append(" ")

    def handle_data(self, data: str) -> None:
        if not self._ignored_depth:
            self.parts.append(data)


def canonical_html(raw: str, ignored_patterns: Iterable[str] = ()) -> str:
    parser = _VisibleText()
    parser.feed(raw)
    visible = html.unescape("".join(parser.parts))
    for pattern in ignored_patterns:
        visible = visible.replace(pattern, "")
    return " ".join(visible.split())


def _decode_pointer_part(value: str) -> str:
    return value.replace("~1", "/").replace("~0", "~")


def remove_json_pointer(value: Any, pointer: str) -> None:
    parts = [_decode_pointer_part(part) for part in pointer.split("/")[1:]]
    if not parts:
        return
    current = value
    for part in parts[:-1]:
        if isinstance(current, dict) and part in current:
            current = current[part]
        elif isinstance(current, list) and part.isdigit() and int(part) < len(current):
            current = current[int(part)]
        else:
            return
    last = parts[-1]
    if isinstance(current, dict):
        current.pop(last, None)
    elif isinstance(current, list) and last.isdigit() and int(last) < len(current):
        current.pop(int(last))


def canonical_json(raw: str, ignored_pointers: Iterable[str] = ()) -> str:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise LedgerError(f"malformed structured JSON: {error.msg}") from error
    for pointer in ignored_pointers:
        remove_json_pointer(value, pointer)
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def canonical_csv(raw: str, options: dict[str, Any]) -> str:
    delimiter = options.get("delimiter", ",")
    quotechar = options.get("quotechar", '"')
    key_columns = options.get("key_columns", [])
    ignored_columns = set(options.get("ignored_columns", []))
    if len(key_columns) != 1:
        raise LedgerError("tabular CSV requires exactly one key column")
    try:
        rows = list(
            csv.reader(
                io.StringIO(raw.removeprefix("\ufeff"), newline=""),
                delimiter=delimiter,
                quotechar=quotechar,
                strict=True,
            )
        )
    except csv.Error as error:
        raise LedgerError(f"malformed tabular CSV: {error}") from error
    if not rows:
        raise LedgerError("tabular CSV requires a header row")
    header = rows[0]
    if not header or any(not column for column in header):
        raise LedgerError("tabular CSV header names must be non-empty")
    duplicates = sorted({column for column in header if header.count(column) > 1})
    if duplicates:
        raise LedgerError(f"duplicate tabular CSV header: {duplicates[0]}")
    key_column = key_columns[0]
    if key_column not in header:
        raise LedgerError(f"tabular CSV missing key column: {key_column}")
    semantic_columns = [column for column in header if column not in ignored_columns]
    canonical_rows: dict[str, dict[str, str]] = {}
    for ordinal, row in enumerate(rows[1:], start=2):
        if not row:
            continue
        if len(row) != len(header):
            raise LedgerError(
                f"tabular CSV row {ordinal} has {len(row)} cells; expected {len(header)}"
            )
        mapped = dict(zip(header, row, strict=True))
        key = mapped[key_column]
        if not key:
            raise LedgerError(f"tabular CSV row {ordinal} has an empty key")
        if key in canonical_rows:
            raise LedgerError(f"duplicate tabular CSV key: {key}")
        canonical_rows[key] = {
            column: mapped[column] for column in semantic_columns
        }
    canonical = {
        "schema": {column: "string" for column in semantic_columns},
        "rows": canonical_rows,
    }
    return json.dumps(canonical, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def flatten_json(value: Any, prefix: str = "") -> dict[str, str]:
    output: dict[str, str] = {}
    if isinstance(value, dict):
        output[prefix] = "@container:object"
        for key in sorted(value):
            escaped = key.replace("~", "~0").replace("/", "~1")
            output.update(flatten_json(value[key], f"{prefix}/{escaped}"))
    elif isinstance(value, list):
        output[prefix] = "@container:array"
        for index, item in enumerate(value):
            output.update(flatten_json(item, f"{prefix}/{index}"))
    else:
        output[prefix] = json.dumps(value, sort_keys=True, ensure_ascii=False)
    return output


def text_fragments(value: str) -> list[str]:
    fragments: list[str] = []
    current: list[str] = []
    for character in value:
        current.append(character)
        if character in ".!?\n":
            fragment = "".join(current).strip()
            if fragment:
                fragments.append(fragment)
            current = []
    fragment = "".join(current).strip()
    if fragment:
        fragments.append(fragment)
    return fragments


def default_ledger() -> dict[str, Any]:
    return {
        "schema": LEDGER_SCHEMA,
        "scheduler_state": "unarmed",
        "sources": {},
        "versions": {},
        "proposals": [],
        "review_responses": [],
        "claim_history": {},
        "supersessions": [],
    }


@dataclass
class CaptureResult:
    source_id: str
    version_id: str
    materiality: str
    reason_codes: list[str]
    affected_claim_ids: list[str]
    proposal_id: str | None
    accepted_conclusions_changed: bool

    def as_dict(self) -> dict[str, Any]:
        return copy.deepcopy(self.__dict__)


class LivingResearchLedger:
    def __init__(self, state: dict[str, Any] | None = None) -> None:
        self.state = copy.deepcopy(state or default_ledger())
        if self.state.get("schema") != LEDGER_SCHEMA:
            raise LedgerError(f"ledger schema must be {LEDGER_SCHEMA}")
        if self.state.get("scheduler_state") != "unarmed":
            raise LedgerError("manual ledger scheduler_state must remain unarmed")
        self.state.setdefault("review_responses", [])

    @classmethod
    def load(cls, path: Path) -> "LivingResearchLedger":
        return cls(json.loads(path.read_text(encoding="utf-8")))

    def save(self, path: Path, *, acquire_lock: bool = True) -> None:
        if acquire_lock:
            with ledger_write_lock(path):
                self.save(path, acquire_lock=False)
            return
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_name(
            f".{path.name}.{os.getpid()}.{uuid.uuid4().hex}.tmp"
        )
        try:
            temporary.write_text(
                json.dumps(self.state, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)

    def register_source(self, registration: dict[str, Any], registered_at: str) -> None:
        registered_at = require_iso_timestamp(registered_at, "registered_at")
        required = {
            "source_id",
            "name",
            "source_kind",
            "canonical_locator",
            "research_package_id",
            "trust",
            "freshness",
        }
        missing = sorted(required - registration.keys())
        if missing:
            raise LedgerError(f"source registration missing: {', '.join(missing)}")
        source_id = registration["source_id"]
        if source_id in self.state["sources"]:
            raise LedgerError(f"source already registered: {source_id}")
        if registration["source_kind"] not in {
            "html_document",
            "structured_json",
            "tabular_csv",
        }:
            raise LedgerError(
                "source_kind must be html_document, structured_json, or tabular_csv"
            )
        freshness = registration["freshness"]
        if not freshness.get("max_age_hours") or not freshness.get("expected_update_pattern"):
            raise LedgerError("freshness requires max_age_hours and expected_update_pattern")
        normalized = {
            "locator_aliases": [],
            "cosmetic_json_pointers": [],
            "cosmetic_text_patterns": [],
            "claim_bindings": [],
            "review_policy": {
                "required_independent_reviewers": 1,
                "approval_rule": "single",
            },
            **copy.deepcopy(registration),
        }
        self._validate_source_options(normalized)
        self.state["sources"][source_id] = normalized
        self.state["versions"][source_id] = []
        for binding in normalized["claim_bindings"]:
            claim_id = binding["claim_id"]
            revision = {
                "revision_id": stable_id(
                    "claim-revision",
                    f"{claim_id}|{registered_at}|{binding['accepted_conclusion']}",
                ),
                "claim_id": claim_id,
                "conclusion": binding["accepted_conclusion"],
                "accepted_at": registered_at,
                "accepted_by": "initial-registration",
                "proposal_id": None,
                "supersedes_revision_id": None,
            }
            self.state["claim_history"].setdefault(claim_id, []).append(revision)

    def _validate_source_options(self, registration: dict[str, Any]) -> None:
        canonical_locator = registration.get("canonical_locator")
        if not isinstance(canonical_locator, str) or not canonical_locator.strip():
            raise LedgerError("canonical_locator must be a non-empty string")
        aliases = registration.get("locator_aliases")
        if not isinstance(aliases, list) or any(
            not isinstance(alias, str) or not alias.strip() for alias in aliases
        ):
            raise LedgerError("locator_aliases must be non-empty strings")
        if len(aliases) != len(set(aliases)):
            raise LedgerError("locator_aliases must be unique")
        if canonical_locator in aliases:
            raise LedgerError("locator_aliases cannot repeat canonical_locator")
        policy = registration["review_policy"]
        required_reviewers = policy.get("required_independent_reviewers")
        if (
            isinstance(required_reviewers, bool)
            or not isinstance(required_reviewers, int)
            or required_reviewers < 1
        ):
            raise LedgerError("review policy requires a positive reviewer count")
        if policy.get("approval_rule") not in {"single", "all_approve"}:
            raise LedgerError("review approval_rule must be single or all_approve")
        if registration["source_kind"] != "tabular_csv":
            return
        options = registration.get("tabular_csv")
        if not isinstance(options, dict):
            raise LedgerError("tabular_csv source requires tabular_csv options")
        if options.get("header") is not True:
            raise LedgerError("tabular CSV requires a header row")
        if options.get("encoding", "utf-8-sig") not in {"utf-8", "utf-8-sig"}:
            raise LedgerError("tabular CSV encoding must be utf-8 or utf-8-sig")
        for field, default in (("delimiter", ","), ("quotechar", '"')):
            value = options.get(field, default)
            if not isinstance(value, str) or len(value) != 1:
                raise LedgerError(f"tabular CSV {field} must be one character")
        key_columns = options.get("key_columns")
        if not isinstance(key_columns, list) or len(key_columns) != 1:
            raise LedgerError("tabular CSV requires exactly one key column")
        if not isinstance(key_columns[0], str) or not key_columns[0]:
            raise LedgerError("tabular CSV key column must be non-empty")
        ignored = options.get("ignored_columns", [])
        if not isinstance(ignored, list) or any(
            not isinstance(column, str) or not column for column in ignored
        ):
            raise LedgerError("tabular CSV ignored_columns must be strings")
        if len(ignored) != len(set(ignored)):
            raise LedgerError("tabular CSV ignored_columns must be unique")
        if key_columns[0] in ignored:
            raise LedgerError("tabular CSV key column cannot be ignored")

    def current_conclusions(self) -> dict[str, str]:
        return {
            claim_id: history[-1]["conclusion"]
            for claim_id, history in self.state["claim_history"].items()
            if history
        }

    def pending_proposals(self) -> list[dict[str, Any]]:
        return [
            copy.deepcopy(proposal)
            for proposal in self.state["proposals"]
            if proposal["status"] in {"pending", "deferred"}
        ]

    def capture(self, packet: dict[str, Any]) -> CaptureResult:
        if packet.get("schema") != CAPTURE_SCHEMA:
            raise LedgerError(f"capture schema must be {CAPTURE_SCHEMA}")
        source_id = packet.get("source_id")
        registration = self.state["sources"].get(source_id)
        if not registration:
            raise LedgerError(f"source not registered: {source_id}")
        if packet.get("source_kind") != registration["source_kind"]:
            raise LedgerError("capture source_kind does not match registration")
        observed_at = require_iso_timestamp(packet.get("observed_at"), "observed_at")
        locator = packet.get("locator")
        if not isinstance(locator, str) or not locator.strip():
            raise LedgerError("capture locator must be a non-empty string")
        canonical_locator = registration["canonical_locator"]
        locator_aliases = registration.get("locator_aliases", [])
        if locator not in {canonical_locator, *locator_aliases}:
            raise LedgerError("capture locator is not registered for this source")
        status = packet.get("status")
        if status not in {
            "observed",
            "deleted",
            "retracted",
            "inaccessible",
            "rate_limited",
            "malformed",
            "relocated",
            "restored",
        }:
            raise LedgerError(f"unsupported capture status: {status}")
        if status == "relocated" and locator not in locator_aliases:
            raise LedgerError("relocated capture must use a registered locator alias")

        accepted_before = self.current_conclusions()
        history = self.state["versions"][source_id]
        previous = history[-1] if history else None
        canonical = self._canonicalize(registration, packet)
        content_identity = (
            stable_id("content", canonical) if canonical is not None else None
        )
        ordinal = len(history) + 1
        version_id = stable_id(
            "source-version",
            "|".join(
                [
                    source_id,
                    str(ordinal),
                    observed_at,
                    content_identity or "no-content",
                    status,
                ]
            ),
        )
        version = {
            "version_id": version_id,
            "source_id": source_id,
            "ordinal": ordinal,
            "locator": packet["locator"],
            "observed_at": observed_at,
            "declared_version": packet.get("declared_version"),
            "status": status,
            "content_identity": content_identity,
            "canonical_content": canonical,
            "raw_content": packet.get("content"),
            "response_status": packet.get("response_status"),
            "retry_after_seconds": packet.get("retry_after_seconds"),
            "extraction_method": packet.get("extraction_method", "unknown"),
            "extraction_warnings": packet.get("extraction_warnings", []),
        }
        change = self._classify(registration, previous, version)
        history.append(version)
        impacts = self._claim_impacts(registration, change)
        proposal_id = None
        if change["materiality"] in REVIEW_REQUIRED:
            proposal_id = stable_id("proposal", f"{source_id}|{version_id}")
            proposal = {
                "schema": PROPOSAL_SCHEMA,
                "proposal_id": proposal_id,
                "source_id": source_id,
                "research_package_id": registration["research_package_id"],
                "created_at": observed_at,
                "status": "pending",
                "materiality": change["materiality"],
                "why_proposed": change["reason_codes"],
                "changed_selectors": change["changed_selectors"],
                "affected_claims": impacts,
                "before_version_id": previous["version_id"] if previous else None,
                "after_version_id": version_id,
                "reviewer_effort_points": 1
                + len(impacts)
                + int(change["materiality"] == "critical"),
                "review_policy": copy.deepcopy(registration["review_policy"]),
                "review_state": "awaiting_responses",
                "reviewed_by": None,
                "reviewed_at": None,
                "review_rationale": None,
            }
            self.state["proposals"].append(proposal)
        accepted_after = self.current_conclusions()
        return CaptureResult(
            source_id=source_id,
            version_id=version_id,
            materiality=change["materiality"],
            reason_codes=change["reason_codes"],
            affected_claim_ids=[impact["claim_id"] for impact in impacts],
            proposal_id=proposal_id,
            accepted_conclusions_changed=accepted_before != accepted_after,
        )

    def review(self, decision: dict[str, Any]) -> None:
        proposal = next(
            (
                item
                for item in self.state["proposals"]
                if item["proposal_id"] == decision.get("proposal_id")
            ),
            None,
        )
        if not proposal:
            raise LedgerError(f"proposal not found: {decision.get('proposal_id')}")
        if proposal["status"] not in {"pending", "deferred"}:
            raise LedgerError("proposal is not reviewable")
        reviewer = require_text(decision.get("reviewer"), "reviewer")
        reviewed_at = require_iso_timestamp(decision.get("reviewed_at"), "reviewed_at")
        rationale = require_text(decision.get("rationale"), "rationale")
        approved = decision.get("approved_conclusions")
        outcome = decision.get("outcome")
        if outcome is None:
            outcome = "approved" if approved is not None else "rejected"
        if outcome not in {"approved", "rejected", "deferred"}:
            raise LedgerError("review outcome must be approved, rejected, or deferred")
        if outcome == "approved" and approved is None:
            raise LedgerError("approved review requires approved_conclusions")
        if outcome != "approved" and approved is not None:
            raise LedgerError(f"{outcome} review cannot include approved_conclusions")
        if approved is not None:
            approved = validate_approved_conclusions(approved)
        policy = proposal.get(
            "review_policy",
            {"required_independent_reviewers": 1, "approval_rule": "single"},
        )
        if policy["required_independent_reviewers"] > 1:
            summary = self.review_response_summary(proposal["proposal_id"])
            if summary["distinct_reviewer_count"] < policy["required_independent_reviewers"]:
                raise LedgerError("independent reviewer quorum has not been reached")
            if outcome == "approved":
                if summary["review_state"] != "quorum_reached":
                    raise LedgerError("independent reviewer responses do not approve")
                if approved != summary["approved_conclusions_consensus"]:
                    raise LedgerError("final approval must match reviewer consensus")
        if approved is not None:
            affected = {impact["claim_id"] for impact in proposal["affected_claims"]}
            unrelated = sorted(set(approved) - affected)
            if unrelated:
                raise LedgerError(f"approval contains unrelated claim: {unrelated[0]}")
            for claim_id, conclusion in approved.items():
                history = self.state["claim_history"][claim_id]
                prior = history[-1]
                revision_id = stable_id(
                    "claim-revision",
                    f"{claim_id}|{reviewed_at}|{reviewer}|{conclusion}",
                )
                history.append(
                    {
                        "revision_id": revision_id,
                        "claim_id": claim_id,
                        "conclusion": conclusion,
                        "accepted_at": reviewed_at,
                        "accepted_by": reviewer,
                        "proposal_id": proposal["proposal_id"],
                        "supersedes_revision_id": prior["revision_id"],
                    }
                )
                self.state["supersessions"].append(
                    {
                        "supersession_id": stable_id(
                            "supersession",
                            f"{claim_id}|{prior['revision_id']}|{revision_id}",
                        ),
                        "claim_id": claim_id,
                        "prior_revision_id": prior["revision_id"],
                        "new_revision_id": revision_id,
                        "proposal_id": proposal["proposal_id"],
                        "reviewer": reviewer,
                        "rationale": rationale,
                        "decided_at": reviewed_at,
                    }
                )
            proposal["status"] = "approved"
        else:
            proposal["status"] = outcome
        proposal["reviewed_by"] = reviewer
        proposal["reviewed_at"] = reviewed_at
        proposal["review_rationale"] = rationale

    def record_review_response(self, response: dict[str, Any]) -> dict[str, Any]:
        if response.get("schema") != REVIEW_RESPONSE_SCHEMA:
            raise LedgerError(f"review response schema must be {REVIEW_RESPONSE_SCHEMA}")
        proposal = next(
            (
                item
                for item in self.state["proposals"]
                if item["proposal_id"] == response.get("proposal_id")
            ),
            None,
        )
        if not proposal:
            raise LedgerError(f"proposal not found: {response.get('proposal_id')}")
        if proposal["status"] not in {"pending", "deferred"}:
            raise LedgerError("proposal is not reviewable")
        reviewer = require_text(response.get("reviewer"), "review response reviewer")
        if any(
            item["proposal_id"] == proposal["proposal_id"]
            and item["reviewer"] == reviewer
            for item in self.state["review_responses"]
        ):
            raise LedgerError("reviewer already responded to this proposal")
        outcome = response.get("outcome")
        if outcome not in {"approved", "rejected", "deferred"}:
            raise LedgerError("review response outcome must be approved, rejected, or deferred")
        rationale = require_text(response.get("rationale"), "review response rationale")
        reviewed_at = require_iso_timestamp(
            response.get("reviewed_at"), "review response reviewed_at"
        )
        approved = response.get("approved_conclusions")
        if outcome == "approved" and not approved:
            raise LedgerError("approved response requires approved_conclusions")
        if outcome != "approved" and approved is not None:
            raise LedgerError(f"{outcome} response cannot include approved_conclusions")
        if approved is not None:
            approved = validate_approved_conclusions(approved)
            affected = {impact["claim_id"] for impact in proposal["affected_claims"]}
            unrelated = sorted(set(approved) - affected)
            if unrelated:
                raise LedgerError(
                    f"review response contains unrelated claim: {unrelated[0]}"
                )
        stored = {
            "schema": REVIEW_RESPONSE_SCHEMA,
            "response_id": stable_id(
                "review-response", f"{proposal['proposal_id']}|{reviewer}"
            ),
            "proposal_id": proposal["proposal_id"],
            "reviewer": reviewer,
            "reviewed_at": reviewed_at,
            "outcome": outcome,
            "rationale": rationale,
            "understandable": response.get("understandable"),
            "materially_relevant": response.get("materially_relevant"),
            "approved_conclusions": copy.deepcopy(approved),
        }
        self.state["review_responses"].append(stored)
        summary = self.review_response_summary(proposal["proposal_id"])
        proposal["review_state"] = summary["review_state"]
        return summary

    def review_response_summary(self, proposal_id: str) -> dict[str, Any]:
        proposal = next(
            (item for item in self.state["proposals"] if item["proposal_id"] == proposal_id),
            None,
        )
        if not proposal:
            raise LedgerError(f"proposal not found: {proposal_id}")
        responses = [
            copy.deepcopy(item)
            for item in self.state["review_responses"]
            if item["proposal_id"] == proposal_id
        ]
        policy = proposal.get(
            "review_policy",
            {"required_independent_reviewers": 1, "approval_rule": "single"},
        )
        required = policy["required_independent_reviewers"]
        approved_responses = [
            item for item in responses if item["outcome"] == "approved"
        ]
        consensus: dict[str, str] | None = None
        if approved_responses:
            candidate = approved_responses[0]["approved_conclusions"]
            if all(item["approved_conclusions"] == candidate for item in approved_responses):
                consensus = copy.deepcopy(candidate)
        if len(responses) < required:
            review_state = "awaiting_responses"
        elif any(item["outcome"] == "deferred" for item in responses):
            review_state = "deferred"
        elif (
            policy["approval_rule"] in {"single", "all_approve"}
            and len(approved_responses) == len(responses)
            and consensus is not None
        ):
            review_state = "quorum_reached"
        else:
            review_state = "conflict"
        return {
            "proposal_id": proposal_id,
            "review_policy": copy.deepcopy(policy),
            "review_state": review_state,
            "distinct_reviewer_count": len({item["reviewer"] for item in responses}),
            "responses": responses,
            "approved_conclusions_consensus": consensus
            if review_state == "quorum_reached"
            else None,
        }

    def prepare_review_packet(
        self, proposal_id: str, reviewers: list[str]
    ) -> dict[str, Any]:
        proposal = next(
            (item for item in self.state["proposals"] if item["proposal_id"] == proposal_id),
            None,
        )
        if not proposal:
            raise LedgerError(f"proposal not found: {proposal_id}")
        if any(
            not isinstance(reviewer, str) or not reviewer.strip()
            for reviewer in reviewers
        ):
            raise LedgerError("reviewer packet requires non-empty reviewer identities")
        if len(reviewers) != len(set(reviewers)):
            raise LedgerError("reviewer packet requires distinct reviewer identities")
        policy = proposal.get(
            "review_policy",
            {"required_independent_reviewers": 1, "approval_rule": "single"},
        )
        required = policy["required_independent_reviewers"]
        if len(reviewers) < required:
            raise LedgerError(f"reviewer packet requires at least {required} reviewers")
        versions = self.state["versions"][proposal["source_id"]]
        before = next(
            (
                version
                for version in versions
                if version["version_id"] == proposal["before_version_id"]
            ),
            None,
        )
        after = next(
            version
            for version in versions
            if version["version_id"] == proposal["after_version_id"]
        )
        return {
            "schema": "ResearchMultiReviewerPacketV1",
            "proposal": copy.deepcopy(proposal),
            "source_registration": copy.deepcopy(
                self.state["sources"][proposal["source_id"]]
            ),
            "before_version": copy.deepcopy(before),
            "after_version": copy.deepcopy(after),
            "accepted_conclusions": self.current_conclusions(),
            "instructions": (
                "Review independently. Do not coordinate outcomes. An individual "
                "response never changes an accepted conclusion."
            ),
            "response_templates": [
                {
                    "schema": REVIEW_RESPONSE_SCHEMA,
                    "proposal_id": proposal_id,
                    "reviewer": reviewer,
                    "reviewed_at": None,
                    "outcome": None,
                    "rationale": None,
                    "understandable": None,
                    "materially_relevant": None,
                    "approved_conclusions": None,
                }
                for reviewer in reviewers
            ],
        }

    def _canonicalize(
        self, registration: dict[str, Any], packet: dict[str, Any]
    ) -> str | None:
        if packet["status"] not in CONTENT_STATUSES:
            return None
        content = packet.get("content")
        if content is None:
            raise LedgerError(f"content required for status {packet['status']}")
        if registration["source_kind"] == "html_document":
            return canonical_html(
                content, registration.get("cosmetic_text_patterns", [])
            )
        if registration["source_kind"] == "structured_json":
            return canonical_json(
                content, registration.get("cosmetic_json_pointers", [])
            )
        return canonical_csv(content, registration["tabular_csv"])

    def _classify(
        self,
        registration: dict[str, Any],
        previous: dict[str, Any] | None,
        current: dict[str, Any],
    ) -> dict[str, Any]:
        change = {
            "materiality": "baseline",
            "reason_codes": ["initial_baseline"],
            "changed_selectors": [],
            "added_fragments": [],
            "removed_fragments": [],
        }
        if previous is None:
            return change
        status = current["status"]
        if status == "deleted":
            return {**change, "materiality": "critical", "reason_codes": ["source_deleted"]}
        if status == "retracted":
            return {**change, "materiality": "critical", "reason_codes": ["source_retracted"]}
        if status == "inaccessible":
            return {**change, "materiality": "operational", "reason_codes": ["source_inaccessible"]}
        if status == "rate_limited":
            return {**change, "materiality": "operational", "reason_codes": ["source_rate_limited"]}
        if status == "malformed":
            return {**change, "materiality": "operational", "reason_codes": ["malformed_content"]}
        diff = self._diff(registration, previous, current)
        if status == "restored":
            return {
                **diff,
                "materiality": "critical",
                "reason_codes": [
                    "source_restored",
                    *( ["restoration_content_changed"] if diff["changed"] else [] ),
                ],
            }
        if status == "relocated":
            if previous["content_identity"] == current["content_identity"]:
                return {**change, "materiality": "cosmetic", "reason_codes": ["locator_changed_content_stable"]}
            return {**diff, "materiality": "potentially_material", "reason_codes": ["locator_and_content_changed"]}
        if (
            current.get("declared_version") is not None
            and current.get("declared_version") == previous.get("declared_version")
            and current["content_identity"] != previous["content_identity"]
        ):
            return {**diff, "materiality": "critical", "reason_codes": ["conflicting_same_declared_version"]}
        if previous["content_identity"] == current["content_identity"]:
            reason = (
                "cosmetic_only"
                if previous.get("raw_content") != current.get("raw_content")
                or previous.get("locator") != current.get("locator")
                else "no_change"
            )
            return {
                **change,
                "materiality": "cosmetic" if reason == "cosmetic_only" else "no_change",
                "reason_codes": [reason],
            }
        return {
            **diff,
            "materiality": "potentially_material" if diff["changed"] else "cosmetic",
            "reason_codes": ["semantic_content_changed" if diff["changed"] else "cosmetic_only"],
        }

    def _diff(
        self,
        registration: dict[str, Any],
        previous: dict[str, Any],
        current: dict[str, Any],
    ) -> dict[str, Any]:
        before = previous.get("canonical_content")
        after = current.get("canonical_content")
        if before is None or after is None:
            return {
                "changed": True,
                "changed_selectors": [],
                "added_fragments": [],
                "removed_fragments": [],
            }
        if registration["source_kind"] in {"structured_json", "tabular_csv"}:
            before_flat = flatten_json(json.loads(before))
            after_flat = flatten_json(json.loads(after))
            changed = sorted(
                key
                for key in set(before_flat) | set(after_flat)
                if before_flat.get(key) != after_flat.get(key)
            )
            return {
                "changed": bool(changed),
                "changed_selectors": changed,
                "added_fragments": [],
                "removed_fragments": [],
            }
        before_parts = text_fragments(before)
        after_parts = text_fragments(after)
        matcher = difflib.SequenceMatcher(
            a=before_parts,
            b=after_parts,
            autojunk=False,
        )
        added: list[str] = []
        removed: list[str] = []
        for operation, before_start, before_end, after_start, after_end in matcher.get_opcodes():
            if operation == "equal":
                continue
            removed.extend(before_parts[before_start:before_end])
            added.extend(after_parts[after_start:after_end])
        for fragment in sorted(set(before_parts) & set(after_parts)):
            before_positions = [
                index for index, value in enumerate(before_parts) if value == fragment
            ]
            after_positions = [
                index for index, value in enumerate(after_parts) if value == fragment
            ]
            if before_positions != after_positions:
                if fragment not in removed:
                    removed.append(fragment)
                if fragment not in added:
                    added.append(fragment)
        return {
            "changed": before_parts != after_parts,
            "changed_selectors": [],
            "added_fragments": added,
            "removed_fragments": removed,
        }

    def _claim_impacts(
        self, registration: dict[str, Any], change: dict[str, Any]
    ) -> list[dict[str, Any]]:
        broad = change["materiality"] in {"critical", "operational"}
        impacts: list[dict[str, Any]] = []
        for binding in registration.get("claim_bindings", []):
            selector = binding["selector"]
            matched = broad
            reason = "source-level failure or critical change"
            if selector["kind"] == "json_pointer":
                pointer = selector["pointer"]
                matched = matched or any(
                    changed == pointer or changed.startswith(pointer + "/")
                    for changed in change["changed_selectors"]
                )
                reason = f"changed JSON selector {pointer}"
            elif selector["kind"] == "html_text":
                anchor = selector["anchor"].lower()
                matched = matched or any(
                    anchor in fragment.lower()
                    for fragment in change["added_fragments"]
                    + change["removed_fragments"]
                )
                reason = f"changed text near anchor {selector['anchor']!r}"
            if matched:
                impacts.append(
                    {
                        "claim_id": binding["claim_id"],
                        "research_package_id": registration["research_package_id"],
                        "reason": reason,
                        "accepted_conclusion_at_detection": self.current_conclusions()[
                            binding["claim_id"]
                        ],
                    }
                )
        return impacts


def html_registration() -> dict[str, Any]:
    return {
        "source_id": "source-html-policy",
        "name": "Official policy page",
        "source_kind": "html_document",
        "canonical_locator": "https://standards.example.test/policy",
        "locator_aliases": ["https://standards.example.test/policy-v2"],
        "research_package_id": "package-policy-analysis",
        "trust": "authoritative",
        "freshness": {
            "max_age_hours": 168,
            "expected_update_pattern": "versioned policy with effective date",
        },
        "cosmetic_text_patterns": [],
        "claim_bindings": [
            {
                "claim_id": "claim-html-effective-date",
                "selector": {"kind": "html_text", "anchor": "effective date"},
                "accepted_conclusion": "The policy takes effect January 1.",
            }
        ],
    }


def json_registration() -> dict[str, Any]:
    return {
        "source_id": "source-json-standard",
        "name": "Structured standard dataset",
        "source_kind": "structured_json",
        "canonical_locator": "https://data.example.test/standard.json",
        "locator_aliases": [],
        "research_package_id": "package-threshold-analysis",
        "trust": "primary",
        "freshness": {
            "max_age_hours": 24,
            "expected_update_pattern": "declared integer version",
        },
        "cosmetic_json_pointers": ["/generated_at"],
        "claim_bindings": [
            {
                "claim_id": "claim-json-threshold",
                "selector": {"kind": "json_pointer", "pointer": "/threshold"},
                "accepted_conclusion": "The authoritative threshold is 5.",
            }
        ],
    }


def csv_registration() -> dict[str, Any]:
    return {
        "source_id": "source-csv-thresholds",
        "name": "Versioned regional threshold dataset",
        "source_kind": "tabular_csv",
        "canonical_locator": "https://data.example.test/regional-thresholds.csv",
        "locator_aliases": [
            "https://data.example.test/regional-thresholds.csv?release=4",
            "https://data.example.test/regional-thresholds.csv?release=5",
        ],
        "research_package_id": "package-regional-threshold-analysis",
        "trust": "primary",
        "freshness": {
            "max_age_hours": 720,
            "expected_update_pattern": "monthly versioned CSV release",
        },
        "tabular_csv": {
            "encoding": "utf-8-sig",
            "delimiter": ",",
            "quotechar": '"',
            "header": True,
            "key_columns": ["region"],
            "ignored_columns": ["generated_at"],
        },
        "review_policy": {
            "required_independent_reviewers": 2,
            "approval_rule": "all_approve",
        },
        "claim_bindings": [
            {
                "claim_id": "claim-csv-north-threshold",
                "selector": {
                    "kind": "json_pointer",
                    "pointer": "/rows/north/threshold",
                },
                "accepted_conclusion": "The North region threshold is 5.",
            }
        ],
    }


def adapt_csv_capture(
    registration: dict[str, Any],
    *,
    csv_path: Path | None,
    observed_at: str,
    declared_version: str | None,
    locator: str | None = None,
    status: str = "observed",
) -> dict[str, Any]:
    if registration.get("source_kind") != "tabular_csv":
        raise LedgerError("CSV adapter requires a tabular_csv registration")
    if status not in {
        "observed",
        "deleted",
        "retracted",
        "inaccessible",
        "rate_limited",
        "malformed",
        "relocated",
        "restored",
    }:
        raise LedgerError(f"unsupported capture status: {status}")
    content: str | None = None
    warnings: list[str] = []
    effective_status = status
    if status in CONTENT_STATUSES:
        if csv_path is None:
            raise LedgerError(f"CSV file required for status {status}")
        try:
            encoding = registration["tabular_csv"].get("encoding", "utf-8-sig")
            content = csv_path.read_text(encoding=encoding)
            canonical_csv(content, registration["tabular_csv"])
        except (LedgerError, OSError, UnicodeError) as error:
            effective_status = "malformed"
            content = None
            warnings = ["csv_adapter_validation_error", str(error)]
    elif status == "malformed":
        warnings = ["csv_adapter_validation_error"]
    response_status = {
        "deleted": 404,
        "inaccessible": 503,
        "rate_limited": 429,
    }.get(effective_status, 200)
    return {
        "schema": CAPTURE_SCHEMA,
        "source_id": registration["source_id"],
        "source_kind": "tabular_csv",
        "locator": locator or registration["canonical_locator"],
        "observed_at": observed_at,
        "status": effective_status,
        "content": content,
        "declared_version": declared_version,
        "response_status": response_status,
        "retry_after_seconds": 60 if effective_status == "rate_limited" else None,
        "extraction_warnings": warnings,
        "extraction_method": "tabular_csv_adapter_manual",
        "source_metadata": {
            "trust": registration["trust"],
            "freshness": registration["freshness"],
        },
    }


def packet(
    source_id: str,
    source_kind: str,
    hour: int,
    status: str,
    locator: str,
    content: str | None,
    declared_version: str | None = None,
) -> dict[str, Any]:
    response = {"inaccessible": 503, "rate_limited": 429, "deleted": 404}.get(
        status, 200
    )
    return {
        "schema": CAPTURE_SCHEMA,
        "source_id": source_id,
        "source_kind": source_kind,
        "locator": locator,
        "observed_at": iso_at(hour),
        "status": status,
        "content": content,
        "declared_version": declared_version,
        "response_status": response,
        "retry_after_seconds": 60 if status == "rate_limited" else None,
        "extraction_warnings": ["invalid_content"] if status == "malformed" else [],
        "extraction_method": f"synthetic_{source_kind}_manual",
    }


def run_qualification() -> dict[str, Any]:
    ledger = LivingResearchLedger()
    ledger.register_source(html_registration(), iso_at(0))
    ledger.register_source(json_registration(), iso_at(0))
    ledger.register_source(csv_registration(), iso_at(0))
    html_url = "https://standards.example.test/policy"
    json_url = "https://data.example.test/standard.json"
    csv_url = "https://data.example.test/regional-thresholds.csv"
    ledger.capture(
        packet(
            "source-html-policy",
            "html_document",
            1,
            "observed",
            html_url,
            "<main><p>Effective date is January 1.</p><p>Threshold remains five.</p></main>",
            "1",
        )
    )
    ledger.capture(
        packet(
            "source-json-standard",
            "structured_json",
            1,
            "observed",
            json_url,
            '{"threshold":5,"status":"active","generated_at":"one"}',
            "1",
        )
    )
    ledger.capture(
        packet(
            "source-csv-thresholds",
            "tabular_csv",
            1,
            "observed",
            csv_url,
            (
                "region,threshold,status,generated_at\n"
                "north,5,active,one\n"
                "south,4,active,one\n"
            ),
            "1",
        )
    )
    initial = ledger.current_conclusions()
    cases = [
        (
            "html_cosmetic_markup",
            "cosmetic",
            packet(
                "source-html-policy",
                "html_document",
                2,
                "observed",
                html_url,
                "<main class='new'> <p>Effective date is January 1.</p><p>Threshold remains five.</p></main>",
                "1",
            ),
        ),
        (
            "html_material_edit",
            "material",
            packet(
                "source-html-policy",
                "html_document",
                3,
                "observed",
                html_url,
                "<main><p>Effective date is February 1.</p><p>Threshold remains five.</p></main>",
                "2",
            ),
        ),
        (
            "source_inaccessible",
            "operational",
            packet("source-html-policy", "html_document", 4, "inaccessible", html_url, None),
        ),
        (
            "source_restoration",
            "material",
            packet(
                "source-html-policy",
                "html_document",
                5,
                "restored",
                html_url,
                "<main><p>Effective date is March 1.</p><p>Threshold remains five.</p></main>",
                "3",
            ),
        ),
        (
            "changed_url_same_content",
            "cosmetic",
            packet(
                "source-html-policy",
                "html_document",
                6,
                "relocated",
                html_url + "-v2",
                "<main><p>Effective date is March 1.</p><p>Threshold remains five.</p></main>",
                "3",
            ),
        ),
        (
            "source_deletion",
            "material",
            packet("source-html-policy", "html_document", 7, "deleted", html_url + "-v2", None, "3"),
        ),
        (
            "deleted_source_restored",
            "material",
            packet(
                "source-html-policy",
                "html_document",
                8,
                "restored",
                html_url + "-v2",
                "<main><p>Effective date is March 1.</p><p>Threshold remains five.</p></main>",
                "3",
            ),
        ),
        (
            "source_retraction",
            "material",
            packet("source-html-policy", "html_document", 9, "retracted", html_url + "-v2", None, "3"),
        ),
        (
            "json_cosmetic_metadata",
            "cosmetic",
            packet(
                "source-json-standard",
                "structured_json",
                2,
                "observed",
                json_url,
                '{ "generated_at": "two", "status": "active", "threshold": 5 }',
                "1",
            ),
        ),
        (
            "json_material_field_change",
            "material",
            packet(
                "source-json-standard",
                "structured_json",
                3,
                "observed",
                json_url,
                '{"threshold":7,"status":"active","generated_at":"three"}',
                "2",
            ),
        ),
        (
            "conflicting_same_version_update",
            "material",
            packet(
                "source-json-standard",
                "structured_json",
                4,
                "observed",
                json_url,
                '{"threshold":9,"status":"active","generated_at":"four"}',
                "2",
            ),
        ),
        (
            "source_rate_limit",
            "operational",
            packet("source-json-standard", "structured_json", 5, "rate_limited", json_url, None),
        ),
        (
            "rate_limited_source_restored",
            "material",
            packet(
                "source-json-standard",
                "structured_json",
                6,
                "restored",
                json_url,
                '{"threshold":9,"status":"active","generated_at":"six"}',
                "3",
            ),
        ),
        (
            "malformed_content",
            "operational",
            packet(
                "source-json-standard",
                "structured_json",
                7,
                "malformed",
                json_url,
                "{not valid json",
            ),
        ),
        (
            "csv_row_reorder_cosmetic_metadata",
            "cosmetic",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                2,
                "observed",
                csv_url,
                (
                    "region,threshold,status,generated_at\r\n"
                    "south,4,active,two\r\n"
                    "north,5,active,two\r\n"
                ),
                "1",
            ),
        ),
        (
            "csv_material_cell_change",
            "material",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                3,
                "observed",
                csv_url,
                (
                    "region,threshold,status,generated_at\n"
                    "north,7,active,three\n"
                    "south,4,active,three\n"
                ),
                "2",
            ),
        ),
        (
            "csv_added_row",
            "material",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                4,
                "observed",
                csv_url,
                (
                    "region,threshold,status,generated_at\n"
                    "north,7,active,four\n"
                    "south,4,active,four\n"
                    "west,6,active,four\n"
                ),
                "3",
            ),
        ),
        (
            "csv_removed_row",
            "material",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                5,
                "observed",
                csv_url,
                (
                    "region,threshold,status,generated_at\n"
                    "north,7,active,five\n"
                    "south,4,active,five\n"
                ),
                "4",
            ),
        ),
        (
            "csv_conflicting_same_version",
            "material",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                6,
                "observed",
                csv_url,
                (
                    "region,threshold,status,generated_at\n"
                    "north,8,active,six\n"
                    "south,4,active,six\n"
                ),
                "4",
            ),
        ),
        (
            "csv_changed_url_same_content",
            "cosmetic",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                7,
                "relocated",
                csv_url + "?release=4",
                (
                    "region,threshold,status,generated_at\n"
                    "south,4,active,seven\n"
                    "north,8,active,seven\n"
                ),
                "4",
            ),
        ),
        (
            "csv_malformed_content",
            "operational",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                8,
                "malformed",
                csv_url + "?release=4",
                None,
                "4",
            ),
        ),
        (
            "csv_restoration",
            "material",
            packet(
                "source-csv-thresholds",
                "tabular_csv",
                9,
                "restored",
                csv_url + "?release=5",
                (
                    "region,threshold,status,generated_at\n"
                    "north,8,active,nine\n"
                    "south,4,active,nine\n"
                ),
                "5",
            ),
        ),
    ]
    scenarios: list[dict[str, Any]] = []
    durations: list[int] = []
    for name, expected, capture in cases:
        started = time.perf_counter_ns()
        result = ledger.capture(capture)
        micros = max(1, (time.perf_counter_ns() - started) // 1_000)
        durations.append(micros)
        proposal_created = result.proposal_id is not None
        passed = {
            "material": result.materiality in MATERIAL and proposal_created,
            "cosmetic": result.materiality in {"cosmetic", "no_change"}
            and not proposal_created,
            "operational": result.materiality == "operational" and proposal_created,
        }[expected]
        scenarios.append(
            {
                "name": name,
                "source_type": capture["source_kind"],
                "expected": expected,
                "materiality": result.materiality,
                "proposal_created": proposal_created,
                "affected_claim_ids": result.affected_claim_ids,
                "changed_selectors": next(
                    (
                        proposal["changed_selectors"]
                        for proposal in reversed(ledger.state["proposals"])
                        if proposal["proposal_id"] == result.proposal_id
                    ),
                    [],
                ),
                "reason_codes": result.reason_codes,
                "assessment_micros": micros,
                "passed": passed,
            }
        )

    changed_without_review = ledger.current_conclusions() != initial
    csv_review_target = next(
        proposal
        for proposal in ledger.state["proposals"]
        if proposal["status"] == "pending"
        and proposal["source_id"] == "source-csv-thresholds"
        and "/rows/north/threshold" in proposal["changed_selectors"]
        and proposal["materiality"] == "potentially_material"
    )
    csv_approved_conclusions = {
        "claim-csv-north-threshold": "The North region threshold is 7."
    }
    for reviewer in ("synthetic-reviewer-a", "synthetic-reviewer-b"):
        ledger.record_review_response(
            {
                "schema": REVIEW_RESPONSE_SCHEMA,
                "proposal_id": csv_review_target["proposal_id"],
                "reviewer": reviewer,
                "reviewed_at": iso_at(19),
                "outcome": "approved",
                "rationale": "Synthetic response used only to exercise quorum enforcement.",
                "understandable": True,
                "materially_relevant": True,
                "approved_conclusions": csv_approved_conclusions,
            }
        )
    csv_review_summary = ledger.review_response_summary(
        csv_review_target["proposal_id"]
    )
    csv_conclusion_changed_by_responses = (
        ledger.current_conclusions()["claim-csv-north-threshold"]
        != initial["claim-csv-north-threshold"]
    )
    review_target = next(
        proposal
        for proposal in ledger.state["proposals"]
        if proposal["status"] == "pending"
        and any(
            impact["claim_id"] == "claim-json-threshold"
            for impact in proposal["affected_claims"]
        )
    )
    ledger.review(
        {
            "proposal_id": review_target["proposal_id"],
            "reviewer": "manual-qualification-reviewer",
            "reviewed_at": iso_at(20),
            "rationale": "Reviewed structured diff and authoritative metadata.",
            "approved_conclusions": {
                "claim-json-threshold": "The authoritative threshold is 7 after manual review."
            },
        }
    )
    material_cases = [scenario for scenario in scenarios if scenario["expected"] == "material"]
    cosmetic_cases = [scenario for scenario in scenarios if scenario["expected"] == "cosmetic"]
    detected_material = sum(
        scenario["materiality"] in MATERIAL and scenario["proposal_created"]
        for scenario in material_cases
    )
    false_alerts = sum(
        scenario["proposal_created"] or scenario["materiality"] in MATERIAL
        for scenario in cosmetic_cases
    )
    proposal_count = sum(scenario["proposal_created"] for scenario in scenarios)
    reviewer_burden = sum(
        proposal["reviewer_effort_points"] for proposal in ledger.state["proposals"]
    )
    affected_count = sum(
        len(proposal["affected_claims"]) for proposal in ledger.state["proposals"]
    )
    metrics = {
        "seeded_material_changes": len(material_cases),
        "detected_material_changes": detected_material,
        "missed_material_changes": len(material_cases) - detected_material,
        "missed_material_rate": (len(material_cases) - detected_material)
        / max(1, len(material_cases)),
        "seeded_cosmetic_changes": len(cosmetic_cases),
        "false_positive_alerts": false_alerts,
        "false_positive_rate": false_alerts / max(1, len(cosmetic_cases)),
        "proposals_created": proposal_count,
        "median_impact_assessment_micros": statistics.median(durations),
        "max_impact_assessment_micros": max(durations),
        "reviewer_burden_points": reviewer_burden,
        "average_affected_claims_per_proposal": affected_count
        / max(1, proposal_count),
    }
    report = {
        "schema": "LivingResearchQualificationReportV1",
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "system_mode": "qualified_manual_unarmed",
        "source_types": ["html_document", "structured_json", "tabular_csv"],
        "scenarios": scenarios,
        "metrics": metrics,
        "accepted_conclusions_changed_without_review": changed_without_review,
        "independent_review_qualification": {
            "evidence_kind": "synthetic_enforcement_only",
            "proposal_id": csv_review_target["proposal_id"],
            "distinct_response_count": csv_review_summary[
                "distinct_reviewer_count"
            ],
            "review_state": csv_review_summary["review_state"],
            "accepted_conclusion_changed_by_responses": csv_conclusion_changed_by_responses,
            "genuine_human_multi_reviewer_pilot": "UNKNOWN_NOT_YET_RUN",
        },
        "approved_review_count": sum(
            proposal["status"] == "approved" for proposal in ledger.state["proposals"]
        ),
        "recoverable_source_version_count": sum(
            len(history) for history in ledger.state["versions"].values()
        ),
        "recoverable_claim_revision_count": sum(
            len(history) for history in ledger.state["claim_history"].values()
        ),
        "scheduler_state": "unarmed",
        "provider_reliability": "UNKNOWN_NOT_EXERCISED",
        "natural_recurrence": "UNKNOWN_NOT_AUTHORIZED",
        "all_scenarios_passed": all(scenario["passed"] for scenario in scenarios)
        and metrics["missed_material_changes"] == 0
        and metrics["false_positive_alerts"] == 0
        and not changed_without_review
        and csv_review_summary["review_state"] == "quorum_reached"
        and not csv_conclusion_changed_by_responses,
    }
    return {"schema": QUALIFICATION_SCHEMA, "report": report, "ledger": ledger.state}


def inspect_state(state: dict[str, Any]) -> dict[str, Any]:
    ledger = state.get("ledger", state)
    return {
        "schema": ledger["schema"],
        "scheduler_state": ledger["scheduler_state"],
        "source_count": len(ledger["sources"]),
        "version_count": sum(len(history) for history in ledger["versions"].values()),
        "pending_proposals": [
            proposal
            for proposal in ledger["proposals"]
            if proposal["status"] in {"pending", "deferred"}
        ],
        "claim_history": ledger["claim_history"],
        "supersessions": ledger["supersessions"],
    }


def _load_json(path: str) -> Any:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    qualify = subparsers.add_parser("qualify")
    qualify.add_argument("--output")
    initialize = subparsers.add_parser("init")
    initialize.add_argument("--registry", required=True)
    initialize.add_argument("--ledger", required=True)
    ingest = subparsers.add_parser("ingest")
    ingest.add_argument("--ledger", required=True)
    ingest.add_argument("--packet", required=True)
    adapt_csv = subparsers.add_parser("adapt-csv")
    adapt_csv.add_argument("--registry", required=True)
    adapt_csv.add_argument("--source-id", required=True)
    adapt_csv.add_argument("--csv")
    adapt_csv.add_argument("--observed-at", required=True)
    adapt_csv.add_argument("--declared-version")
    adapt_csv.add_argument("--locator")
    adapt_csv.add_argument(
        "--status",
        default="observed",
        choices=[
            "observed",
            "deleted",
            "retracted",
            "inaccessible",
            "rate_limited",
            "malformed",
            "relocated",
            "restored",
        ],
    )
    adapt_csv.add_argument("--output", required=True)
    review = subparsers.add_parser("review")
    review.add_argument("--ledger", required=True)
    review.add_argument("--decision", required=True)
    respond = subparsers.add_parser("respond")
    respond.add_argument("--ledger", required=True)
    respond.add_argument("--response", required=True)
    prepare_review = subparsers.add_parser("prepare-review-pilot")
    prepare_review.add_argument("--ledger", required=True)
    prepare_review.add_argument("--proposal-id", required=True)
    prepare_review.add_argument("--reviewer", action="append", required=True)
    prepare_review.add_argument("--output", required=True)
    review_summary = subparsers.add_parser("review-summary")
    review_summary.add_argument("--ledger", required=True)
    review_summary.add_argument("--proposal-id", required=True)
    inspect = subparsers.add_parser("inspect")
    inspect.add_argument("--ledger", required=True)
    args = parser.parse_args(argv)

    if args.command == "qualify":
        output = run_qualification()
        rendered = json.dumps(output, indent=2, sort_keys=True) + "\n"
        if args.output:
            Path(args.output).write_text(rendered, encoding="utf-8")
        else:
            sys.stdout.write(rendered)
        return 0
    if args.command == "init":
        ledger_path = Path(args.ledger)
        with ledger_write_lock(ledger_path):
            if ledger_path.exists():
                raise LedgerError("ledger path already exists")
            ledger = LivingResearchLedger()
            registry = _load_json(args.registry)
            for source in registry["sources"]:
                ledger.register_source(source, registry["registered_at"])
            ledger.save(ledger_path, acquire_lock=False)
        return 0
    if args.command == "ingest":
        ledger_path = Path(args.ledger)
        with ledger_write_lock(ledger_path):
            ledger = LivingResearchLedger.load(ledger_path)
            result = ledger.capture(_load_json(args.packet))
            ledger.save(ledger_path, acquire_lock=False)
        print(json.dumps(result.as_dict(), indent=2, sort_keys=True))
        return 0
    if args.command == "adapt-csv":
        registry = _load_json(args.registry)
        registration = next(
            (
                source
                for source in registry["sources"]
                if source["source_id"] == args.source_id
            ),
            None,
        )
        if registration is None:
            raise LedgerError(f"source not found in registry: {args.source_id}")
        packet_value = adapt_csv_capture(
            registration,
            csv_path=Path(args.csv) if args.csv else None,
            observed_at=args.observed_at,
            declared_version=args.declared_version,
            locator=args.locator,
            status=args.status,
        )
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            json.dumps(packet_value, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        return 0
    if args.command == "review":
        ledger_path = Path(args.ledger)
        with ledger_write_lock(ledger_path):
            ledger = LivingResearchLedger.load(ledger_path)
            ledger.review(_load_json(args.decision))
            ledger.save(ledger_path, acquire_lock=False)
        return 0
    if args.command == "respond":
        ledger_path = Path(args.ledger)
        with ledger_write_lock(ledger_path):
            ledger = LivingResearchLedger.load(ledger_path)
            summary = ledger.record_review_response(_load_json(args.response))
            ledger.save(ledger_path, acquire_lock=False)
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0
    if args.command == "prepare-review-pilot":
        ledger = LivingResearchLedger.load(Path(args.ledger))
        packet_value = ledger.prepare_review_packet(
            args.proposal_id, args.reviewer
        )
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            json.dumps(packet_value, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        return 0
    if args.command == "review-summary":
        ledger = LivingResearchLedger.load(Path(args.ledger))
        print(
            json.dumps(
                ledger.review_response_summary(args.proposal_id),
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    if args.command == "inspect":
        print(json.dumps(inspect_state(_load_json(args.ledger)), indent=2, sort_keys=True))
        return 0
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
