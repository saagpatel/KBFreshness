# KB Freshness Detector

[![Rust](https://img.shields.io/badge/Rust-dea584?style=flat-square&logo=rust)](#) [![TypeScript](https://img.shields.io/badge/TypeScript-3178c6?style=flat-square&logo=typescript)](#) [![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](#)

> Knowledge bases decay silently. This catches the rot before users do.

KB Freshness Detector monitors Confluence knowledge base articles for staleness and broken links, with optional screenshot comparison and Jira ticket correlation. When background automation is enabled, daily freshness scans update the dashboard so documentation teams can prioritize updates rather than audit manually.

## Features

- **Automated freshness tracking** — opt-in daily scans of Confluence spaces with per-article and global staleness thresholds (default 90 days)
- **Broken link detection** — concurrent HTTP checks of absolute and root-relative article links, with HEAD-to-GET fallback
- **Visual drift detection** — opt-in weekly screenshots of article URLs with hash-based comparison (requires the `screenshots` Cargo feature)
- **Support ticket correlation** — Jaro-Winkler similarity and keyword extraction match Jira tickets updated in the last 30 days to article titles (requires the `tickets` Cargo feature)
- **Health dashboard** — article health statuses, status counts, and scan results at a glance
- **AI-powered suggestions** — optional Ollama integration generates update recommendations based on ticket patterns
- **Scan safeguards** — scheduled freshness-scan retries and configurable link-check concurrency

## Quick Start

### Prerequisites

- Node.js 20.19+ on 20.x, 22.12+ on 22.x, or 24+ (locked Vite 8/Vitest 4 requirements)
- Rust stable toolchain (`rustup`)
- PostgreSQL for running the backend; `DATABASE_URL` points to a disposable development database
- Python 3.10+ for the standard-library Living Research fixture suite
- Confluence API credentials only for explicitly requested provider scans
- Ollama (optional, for AI suggestions)

### Installation

```bash
git clone https://github.com/saagpatel/KBFreshness
cd KBFreshness
npm --prefix frontend ci
```

### Usage

```bash
# Frontend only (from the repository root)
npm --prefix frontend run dev
```

The maintained backend is the root Axum Cargo package, not a Tauri shell.
Running `cargo run` requires a disposable PostgreSQL `DATABASE_URL` and
applies migrations. Keep `BACKGROUND_AUTOMATION_ENABLED=false`; starting the
backend or triggering a provider scan is not an offline verification step.
Provider credentials are optional until that provider is explicitly used.

## Verification

Run from the repository root. A focused, provider-free fixture check is:

```bash
python3 -m unittest discover -s tests -p test_living_research.py -k cosmetic
```

The broader ledger suite uses the same command without `-k cosmetic` and writes
only temporary fixture state. Rust's focused scheduler-policy test is
`cargo test --bin kb-freshness-detector background_automation_is_fail_closed`.
Frontend checks are `npm --prefix frontend run test -- --run` (non-watch) and
`npm --prefix frontend run build` (TypeScript plus Vite). No frontend lint script is defined.
Rust formatting/checking is `cargo fmt --all -- --check` and `cargo check`.
No `Cargo.lock` is included in this checkout; Cargo dependency resolution may
require network access, so these are not guaranteed offline checks.

The [canonical full gate](.codex/verify.commands) also includes Rust tests and
performance baselines; inspect database-dependent tests and optional
screenshot/ticket feature prerequisites before selecting them. For UI changes,
use synthetic dashboard responses in a local browser. Do not enable recurrence,
use production databases, invoke Confluence/Notion/Jira/Ollama, or capture real
screenshots merely to verify documentation or local fixture behavior.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend service | Axum |
| Frontend | React, TypeScript, Tailwind CSS |
| Backend | Rust — link validation, screenshot comparison, ticket correlation |
| Similarity | Jaro-Winkler string matching |
| AI suggestions | Ollama (optional) |
| Storage | PostgreSQL |

## Architecture

The Rust backend drives all monitoring work: concurrent HTTP requests for link validation with configurable concurrency, headless capture of article URLs with the `screenshots` feature, and a fuzzy matching pipeline with the `tickets` feature. Daily freshness scans and weekly screenshot/ticket jobs require opt-in background automation. The health dashboard aggregates scan results into health statuses the documentation team can act on. AI suggestions run within ticket analysis after correlation and never affect the deterministic health scores.

## Living Research upgrade

The repository now owns a zero-dependency manual research ledger that consumes
portable PageDiffBookmark capture packets, structured JSON observations, and
explicitly keyed versioned CSV datasets. It preserves every source version,
maps material changes to registered claims, creates inspectable review
proposals, and changes accepted conclusions only after an explicit review with
a supersession record. CSV sources can require append-only independent reviewer
responses before final approval. See
[`docs/LIVING_RESEARCH.md`](docs/LIVING_RESEARCH.md).

Background jobs are fail-closed: `BACKGROUND_AUTOMATION_ENABLED` defaults to
disabled. The Living Research qualification does not arm the scheduler or
prove provider or natural-recurrence reliability.

For controlled local recurrence qualification, the disposable
`tools/living_research_recurrence.py` harness waits for real timer slots and
writes terminal receipts without enabling the application scheduler or calling
providers. Its evidence ceiling is local timer delivery only.

## License

MIT
