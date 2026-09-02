#!/usr/bin/env python3
"""Disposable timer-driven qualification harness for Living Research.

This harness proves only local schedule delivery. It never starts the
application scheduler, calls providers, or changes the ledger's fail-closed
``scheduler_state``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


RECEIPT_SCHEMA = "LocalRecurrenceReceiptV1"
SUMMARY_SCHEMA = "LocalRecurrenceQualificationV1"


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def run_command(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )


def run_recurrence(
    *,
    repo_root: Path,
    registry: Path,
    ledger: Path,
    packets: list[Path],
    receipt_dir: Path,
    interval_seconds: float,
    firing_count: int,
) -> dict[str, Any]:
    if interval_seconds <= 0:
        raise ValueError("interval_seconds must be positive")
    if firing_count < 2:
        raise ValueError("firing_count must be at least two")
    if not packets:
        raise ValueError("at least one packet is required")
    if ledger.exists():
        raise ValueError("ledger path must not already exist")
    if receipt_dir.exists():
        raise ValueError("receipt directory must not already exist")

    tool = repo_root / "tools" / "living_research.py"
    init_result = run_command(
        [
            sys.executable,
            str(tool),
            "init",
            "--registry",
            str(registry),
            "--ledger",
            str(ledger),
        ],
        repo_root,
    )
    if init_result.returncode != 0:
        raise RuntimeError(f"ledger initialization failed: {init_result.stderr[-500:]}")

    scheduler_id = f"local-recurrence-{uuid.uuid4()}"
    started_at = utc_now()
    started_monotonic = time.monotonic()
    receipts: list[dict[str, Any]] = []

    for index in range(firing_count):
        scheduled_monotonic = started_monotonic + interval_seconds * (index + 1)
        delay = max(0.0, scheduled_monotonic - time.monotonic())
        time.sleep(delay)
        fired_monotonic = time.monotonic()
        fired_at = utc_now()
        packet = packets[index % len(packets)]
        result = run_command(
            [
                sys.executable,
                str(tool),
                "ingest",
                "--ledger",
                str(ledger),
                "--packet",
                str(packet),
            ],
            repo_root,
        )
        terminal_status = "pass" if result.returncode == 0 else "fail"
        parsed_result: dict[str, Any] | None = None
        if result.stdout.strip():
            try:
                parsed_result = json.loads(result.stdout)
            except json.JSONDecodeError:
                parsed_result = None
        receipt = {
            "schema": RECEIPT_SCHEMA,
            "scheduler_id": scheduler_id,
            "run_id": f"{scheduler_id}-{index + 1}",
            "ordinal": index + 1,
            "scheduled_offset_seconds": interval_seconds * (index + 1),
            "fired_at": fired_at,
            "lateness_milliseconds": round(
                max(0.0, fired_monotonic - scheduled_monotonic) * 1000, 3
            ),
            "packet_path": str(packet),
            "packet_sha256": sha256_file(packet),
            "ledger_sha256": sha256_file(ledger),
            "terminal_status": terminal_status,
            "exit_code": result.returncode,
            "capture_result": parsed_result,
            "stderr_tail": result.stderr[-500:],
            "external_destination": None,
        }
        write_json(receipt_dir / f"receipt-{index + 1:02d}.json", receipt)
        receipts.append(receipt)
        if result.returncode != 0:
            break

    summary = {
        "schema": SUMMARY_SCHEMA,
        "scheduler_id": scheduler_id,
        "mode": "disposable_local_timer",
        "started_at": started_at,
        "completed_at": utc_now(),
        "configured_interval_seconds": interval_seconds,
        "requested_firings": firing_count,
        "observed_firings": len(receipts),
        "distinct_timer_receipts": len({item["run_id"] for item in receipts}),
        "all_receipts_passed": len(receipts) == firing_count
        and all(item["terminal_status"] == "pass" for item in receipts),
        "ledger_path": str(ledger),
        "ledger_sha256": sha256_file(ledger),
        "receipt_directory": str(receipt_dir),
        "external_effects": [],
        "claim_ceiling": (
            "Controlled local timer recurrence only; application scheduling, "
            "providers, deployment, and production recurrence remain unproved."
        ),
        "receipts": receipts,
    }
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", required=True)
    parser.add_argument("--ledger", required=True)
    parser.add_argument("--packet", action="append", required=True)
    parser.add_argument("--receipt-dir", required=True)
    parser.add_argument("--interval-seconds", type=float, default=60.0)
    parser.add_argument("--firings", type=int, default=2)
    parser.add_argument("--summary-output")
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[1]
    summary = run_recurrence(
        repo_root=repo_root,
        registry=Path(args.registry).resolve(),
        ledger=Path(args.ledger).resolve(),
        packets=[Path(item).resolve() for item in args.packet],
        receipt_dir=Path(args.receipt_dir).resolve(),
        interval_seconds=args.interval_seconds,
        firing_count=args.firings,
    )
    rendered = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_output:
        write_json(Path(args.summary_output).resolve(), summary)
    sys.stdout.write(rendered)
    return 0 if summary["all_receipts_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
