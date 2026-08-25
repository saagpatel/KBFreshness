# Living Research and Discovery Program

This repository owns the manual research ledger and review workflow for the
Living Research and Discovery Program. The program is an upgrade of
KBFreshnessDetector's source identity, freshness, and manual-review model. It
consumes portable PageDiffBookmark capture packets; it is not a third generic
monitor.

## Qualified boundary

The local tool at `tools/living_research.py` owns:

- stable source registration with source kind, canonical locator and aliases,
  expected update pattern, trust, and freshness metadata;
- append-only source versions with raw and canonical content, response state,
  extraction method, warnings, and deterministic identities;
- registered-locator enforcement and timezone-aware observation provenance;
- HTML text normalization, structured-JSON canonicalization, and keyed CSV
  canonicalization with deterministic row identity;
- materiality classes: `baseline`, `no_change`, `cosmetic`, `operational`,
  `potentially_material`, and `critical`;
- claim selectors, claim-impact reasons, and research-package routing;
- pending review proposals that preserve the conclusion accepted at detection;
- append-only independent reviewer responses, explicit final approval or
  rejection, claim revision history, and supersession;
- manual qualification metrics for missed material changes, false alerts,
  assessment time, and reviewer burden.

The tool does not own network fetching, generic alerts, notification delivery,
automatic truth revision, paid providers, production deployment, or scheduling.
The serialized ledger always records `scheduler_state: unarmed`.
Mutating CLI commands hold an advisory per-ledger file lock across load,
validation, mutation, and atomic replacement so concurrent local writers do not
silently lose accepted versions or review responses.

## Disposable local recurrence qualification

`tools/living_research_recurrence.py` provides an explicitly invoked,
timer-driven qualification harness. It waits for real scheduled slots, ingests
registered fixture packets through the normal CLI, and writes one terminal
receipt per firing. The harness leaves the application scheduler and ledger
state unarmed, has no provider or notification path, and proves only controlled
local timer delivery. Two distinct timer-fired receipts are required for a
passing local recurrence result.

## Manual workflow

```bash
python3 tools/living_research.py init \
  --registry examples/living-research/source-registry.json \
  --ledger /tmp/living-research-ledger.json

python3 tools/living_research.py ingest \
  --ledger /tmp/living-research-ledger.json \
  --packet examples/living-research/pagediff-baseline.json

python3 tools/living_research.py ingest \
  --ledger /tmp/living-research-ledger.json \
  --packet examples/living-research/pagediff-material-change.json

python3 tools/living_research.py inspect \
  --ledger /tmp/living-research-ledger.json
```

The second ingest proposes review because the changed sentence intersects the
claim's `effective date` selector. It does not change the accepted conclusion.
A reviewer must submit an explicit decision with reviewer, rationale, time, and
an `approved`, `rejected`, or `deferred` outcome. Approval requires a new
conclusion and appends a supersession record. Deferral preserves the accepted
conclusion and keeps the proposal reviewable.

## Versioned CSV workflow

CSV registrations must declare a header, exactly one stable key column,
UTF-8 encoding, delimiter and quote character, and any non-semantic ignored
columns. Cell values remain strings. Row order, line endings, and changes to an
ignored column do not change content identity. Duplicate headers, duplicate or
empty keys, and rows with the wrong number of cells are malformed.

```bash
python3 tools/living_research.py init \
  --registry examples/living-research/source-registry.json \
  --ledger /tmp/living-research-csv-ledger.json

python3 tools/living_research.py adapt-csv \
  --registry examples/living-research/source-registry.json \
  --source-id source-csv-thresholds \
  --csv examples/living-research/regional-thresholds-v1.csv \
  --observed-at 2026-08-23T01:00:00Z \
  --declared-version 1 \
  --output /tmp/csv-v1-packet.json

python3 tools/living_research.py ingest \
  --ledger /tmp/living-research-csv-ledger.json \
  --packet /tmp/csv-v1-packet.json
```

Repeat `adapt-csv` and `ingest` with
`regional-thresholds-v2.csv`. The resulting selector
`/rows/north/threshold` maps the changed cell to the registered claim; it does
not rewrite the conclusion.

## Independent-review pilot

A source can require multiple independent reviewers. Prepare the packet after
ingest using the proposal ID:

```bash
python3 tools/living_research.py prepare-review-pilot \
  --ledger /tmp/living-research-csv-ledger.json \
  --proposal-id PROPOSAL_ID \
  --reviewer human-reviewer-a \
  --reviewer human-reviewer-b \
  --output /tmp/csv-review-pilot.json
```

Each person reviews independently and returns one
`ResearchReviewerResponseV1`. Record each response with `respond`, then inspect
quorum with `review-summary`. Duplicate reviewer identities are rejected;
deferred or disagreeing responses cannot approve the proposal. Even unanimous
responses do not revise truth: a separate final `review` decision must match
the recorded consensus before the ledger appends a claim revision and
supersession. A generated packet or synthetic response is not evidence that a
genuine human pilot occurred.

## Materiality rules

| Observation | Classification | Review proposal |
| --- | --- | --- |
| Equivalent canonical content | `no_change` or `cosmetic` | No |
| HTML text, non-ignored JSON selector, or keyed CSV cell/row changes | `potentially_material` | Yes |
| Same declared version with conflicting content | `critical` | Yes |
| Deletion or retraction | `critical` | Yes |
| Inaccessible, rate-limited, or malformed response | `operational` | Yes, with no truth rewrite |
| URL relocation with identical content identity | `cosmetic` | No |
| Restoration | `critical` | Yes |

JSON source registrations can name cosmetic pointers such as `/generated_at`.
HTML registrations can name exact cosmetic text patterns. Claim impacts are
created only for registered selectors or, for critical/operational source
states, every claim bound to that source.

## Qualification

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tests -p 'test_living_research.py' -v
python3 tools/living_research.py qualify
```

The synthetic qualification covers an HTML document, structured JSON, and a
versioned CSV dataset. It exercises cosmetic edits, material edits, CSV row
addition and removal, deletion, retraction, inaccessible content, relocation,
rate limiting, malformed content, conflicting same-version updates, restoration,
and multi-reviewer quorum enforcement. This is local/manual evidence only.
Genuine human multi-review, provider reliability, activated scheduling, and
natural recurrence remain `UNKNOWN` until directly observed.

## Consumer seams

The proposal and ledger are intentionally JSON. KBF's current manual CLI is the
first consumer and exposes `why_proposed`, changed selectors, affected claims,
the accepted conclusion at detection, source versions, and supersessions.
SpecCompanion is the strongest next consumer for stable requirement claims;
ArguMap is a useful human synthesis consumer. Neither is modified or activated
by this program's current scope.
