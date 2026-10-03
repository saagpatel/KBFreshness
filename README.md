# KB Freshness Detector

[![Rust](https://img.shields.io/badge/Rust-dea584?style=flat-square&logo=rust)](#) [![TypeScript](https://img.shields.io/badge/TypeScript-3178c6?style=flat-square&logo=typescript)](#) [![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](#)

> Knowledge bases decay silently. This catches the rot before users do.

KB Freshness Detector continuously monitors Confluence knowledge base articles for staleness, broken links, visual drift in referenced screenshots, and unaddressed support tickets. Automated daily scans surface the highest-risk articles so documentation teams can prioritize updates rather than audit manually.

## Features

- **Automated freshness tracking** — daily scans of Confluence spaces with per-article and global staleness thresholds (default 90 days)
- **Broken link detection** — concurrent validation of all article links with redirect chain handling and SSL checks
- **Visual drift detection** — weekly screenshots of referenced application UIs with hash-based comparison to catch interface changes
- **Support ticket correlation** — Jaro-Winkler similarity and keyword extraction match open tickets to the articles that should address them
- **Health dashboard** — article health scores, link validity rates, and freshness distributions at a glance
- **AI-powered suggestions** — optional Ollama integration generates update recommendations based on ticket patterns
- **Production hardened** — comprehensive error handling, retry logic, configurable rate limiting

## Quick Start

### Prerequisites

- Node.js 20.19+ (Vite 8 requirement)
- Rust stable toolchain (`rustup`)
- PostgreSQL for running the backend; `DATABASE_URL` points to a disposable development database
- Python 3 for the standard-library Living Research fixture suite
- Confluence API credentials only for explicitly requested provider scans
- Ollama (optional, for AI suggestions)

### Installation

```bash
git clone https://github.com/saagpatel/KBFreshness
cd KBFreshness
npm --prefix frontend ci
cargo metadata --locked --no-deps --format-version 1 >/dev/null
```

The application commits `Cargo.lock` so Rust setup uses the reviewed dependency resolution. Keep it alongside `Cargo.toml`; do not remove it or regenerate it during routine Cloud setup. The metadata command checks the locked package graph without starting the backend or requiring a database.

### Usage

```bash
# Frontend only (from the repository root)
npm --prefix frontend run dev
```

The maintained backend is the root Axum Cargo package, not a Tauri shell.
Running `cargo run --locked` requires a disposable PostgreSQL `DATABASE_URL` and
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
`cargo test --locked --bin kb-freshness-detector background_automation_is_fail_closed`.
Frontend checks are `npm --prefix frontend run test -- --run` (non-watch) and
`npm --prefix frontend run build` (TypeScript plus Vite). No frontend lint script is defined.
Rust formatting/checking is `cargo fmt --all -- --check` and `cargo check --locked`.

The [canonical full gate](.codex/verify.commands) also includes Rust tests and
performance baselines; inspect database-dependent/ignored tests and optional
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

The Rust backend drives all monitoring work: concurrent HTTP requests for link validation with configurable rate limiting, headless screenshot capture on a weekly schedule, and a fuzzy matching pipeline for ticket correlation. The health dashboard aggregates scan results into scores the documentation team can act on. AI suggestions run as a separate pass after correlation and never affect the deterministic health scores.

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
