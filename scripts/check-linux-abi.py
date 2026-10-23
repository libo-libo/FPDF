#!/usr/bin/env python3
"""Check that Linux release binaries fit the AlmaLinux 8 ABI baseline."""

import re
import subprocess
import sys
from pathlib import Path

MAXIMUMS = {
    "GLIBC": (2, 28),
    "GLIBCXX": (3, 4, 25),
    "CXXABI": (1, 3, 11),
}
PATTERN = re.compile(r"\b(GLIBC|GLIBCXX|CXXABI)_([0-9]+(?:\.[0-9]+)+)\b")


def version(value: str) -> tuple[int, ...]:
    return tuple(int(part) for part in value.split("."))


def main() -> None:
    failed = False
    for argument in sys.argv[1:]:
        path = Path(argument)
        header = path.read_bytes()[:20]
        if header[:4] != b"\x7fELF" or header[4] != 2 or int.from_bytes(header[18:20], "little") != 62:
            raise SystemExit(f"Not an ELF64 x86-64 file: {path}")
        output = subprocess.check_output(
            ["readelf", "--version-info", "-W", str(path)], text=True, errors="replace"
        )
        found: dict[str, tuple[int, ...]] = {}
        for family, value in PATTERN.findall(output):
            found[family] = max(found.get(family, ()), version(value))
        summary = ", ".join(
            f"{family} {'.'.join(map(str, found.get(family, ()))) or 'none'}"
            for family in MAXIMUMS
        )
        print(f"{path}: {summary}")
        for family, maximum in MAXIMUMS.items():
            actual = found.get(family)
            if actual and actual > maximum:
                print(
                    f"ERROR: {path} requires {family}_{'.'.join(map(str, actual))}; "
                    f"baseline allows {family}_{'.'.join(map(str, maximum))}",
                    file=sys.stderr,
                )
                failed = True
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
