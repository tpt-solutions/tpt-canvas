"""Fails when a benchmark is slower than its saved baseline by more than a threshold.

Criterion writes each run to target/criterion/<bench>/<name>/estimates.json, and the run that
was just made to target/criterion/<bench>/new/estimates.json. This script compares the mean of
every bench that has both.

Usage: python scripts/bench_compare.py <baseline-name> [--threshold 0.25] [--root target/criterion]

Exit status: 0 when every bench is within the threshold, 1 when one is slower, and 2 when no
bench had both files (so a missing baseline cannot pass silently).
"""

import argparse
import json
import pathlib
import sys


def mean_ns(path: pathlib.Path) -> float:
    with path.open(encoding="utf-8") as f:
        return float(json.load(f)["mean"]["point_estimate"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("baseline", help="name given to --save-baseline")
    parser.add_argument("--threshold", type=float, default=0.25,
                        help="allowed slowdown as a fraction (0.25 = 25%%)")
    parser.add_argument("--root", default="target/criterion", help="criterion output directory")
    args = parser.parse_args()

    root = pathlib.Path(args.root)
    rows = []
    for base in sorted(root.glob(f"**/{args.baseline}/estimates.json")):
        bench_dir = base.parent.parent
        new = bench_dir / "new" / "estimates.json"
        if not new.exists():
            continue
        before, after = mean_ns(base), mean_ns(new)
        ratio = after / before if before > 0 else float("inf")
        rows.append((bench_dir.relative_to(root).as_posix(), before, after, ratio))

    if not rows:
        print(f"no benchmarks with baseline '{args.baseline}' under {root}", file=sys.stderr)
        return 2

    width = max(len(name) for name, *_ in rows)
    failed = []
    print(f"{'benchmark':<{width}}  {'baseline ns':>14}  {'now ns':>14}  change")
    for name, before, after, ratio in rows:
        slow = ratio > 1.0 + args.threshold
        if slow:
            failed.append(name)
        print(f"{name:<{width}}  {before:>14.0f}  {after:>14.0f}  {ratio - 1:+8.1%}"
              f"{'  FAIL' if slow else ''}")

    if failed:
        print(f"\n{len(failed)} benchmark(s) more than {args.threshold:.0%} slower than "
              f"'{args.baseline}'", file=sys.stderr)
        return 1
    print(f"\nall {len(rows)} benchmarks within {args.threshold:.0%} of '{args.baseline}'")
    return 0


if __name__ == "__main__":
    sys.exit(main())
