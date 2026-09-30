#!/usr/bin/env python3
"""Survey installed or expected zones in isolated CPU-only processes; never certifies fidelity."""
import argparse
import concurrent.futures
import json
import hashlib
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/zone_audit"))
    parser.add_argument("--dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=2, choices=range(1, 5))
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--metadata-only", action="store_true")
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--zones", nargs="+")
    selection.add_argument("--zones-file", type=Path,
                           help="expected zone short names, one per line; blank lines and # comments allowed")
    args = parser.parse_args()
    if not 0 < args.timeout <= 3600:
        parser.error("timeout must be between 0 and 3600 seconds")
    binary = args.binary.resolve(strict=True)
    base = args.dir.resolve(strict=True)
    zones = args.zones
    if args.zones_file:
        zones = [line.split("#", 1)[0].strip() for line in args.zones_file.read_text().splitlines()]
        zones = [zone for zone in zones if zone]
        if not zones:
            parser.error("zones file contains no zone names")
    if zones:
        if any(not zone.isascii() or not all(c.isalnum() or c == "_" for c in zone) for zone in zones):
            parser.error("zone names must contain only ASCII letters, numbers or underscores")
        manifest = {"zones": sorted({zone.lower() for zone in zones}),
                    "scope": "expected_zone_list" if args.zones_file else "explicit_selection"}
        if args.zones_file:
            manifest["zones_file"] = str(args.zones_file.resolve())
    else:
        discovery = subprocess.run([str(binary), "--list", "--dir", str(base)],
                                   check=True, capture_output=True, text=True, timeout=300)
        manifest = json.loads(discovery.stdout)
        manifest["scope"] = "installed_declarations"
    args.output.mkdir(parents=True, exist_ok=False)
    with binary.open("rb") as executable:
        binary_hash = hashlib.file_digest(executable, "sha256").hexdigest()
    manifest.update({"binary": str(binary), "binary_sha256": binary_hash, "asset_directory": str(base),
                     "metadata_only": args.metadata_only, "timeout_seconds": args.timeout})
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    def inspect(zone):
        command = [str(binary), zone, "--dir", str(base)]
        if args.metadata_only:
            command.append("--metadata-only")
        start = time.monotonic()
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=args.timeout)
            try:
                report = json.loads(result.stdout)
                if not isinstance(report, dict) or report.get("zone") != zone.lower():
                    raise ValueError("unexpected zone report")
            except (ValueError, TypeError):
                report = {"zone": zone, "report_error": "missing or invalid JSON result"}
            report["exit_code"] = result.returncode
            if result.stderr:
                report["stderr"] = result.stderr[-8192:]
            return report
        except subprocess.TimeoutExpired:
            return {"zone": zone, "timeout": True, "seconds": time.monotonic() - start}

    reports = []
    with (args.output / "zones.jsonl").open("w") as output:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
            futures = [executor.submit(inspect, zone) for zone in manifest["zones"]]
            for future in concurrent.futures.as_completed(futures):
                report = future.result()
                output.write(json.dumps(report, sort_keys=True) + "\n")
                output.flush()
                reports.append(report)
    counts = {}
    for report in reports:
        for field in ("format", "liquids", "borders"):
            value = report.get("metadata", {}).get(field, "metadata_failed")
            key = field + ":" + value
            counts[key] = counts.get(key, 0) + 1
    summary = {"zones_attempted": len(reports), "counts": counts,
               "missing_assets": sorted(r["zone"] for r in reports if r.get("missing_assets")),
               "failed_or_timed_out": [r["zone"] for r in reports if r.get("exit_code") != 0],
               "limits": "CPU structure/metadata only; textures, GPU appearance, traversability, NPCs and audio not certified"}
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
