#!/usr/bin/env python3
"""Collect license texts for the active Rust dependency graph without network access."""

import argparse
import json
import re
import subprocess
from pathlib import Path


def command(*args: str) -> str:
    return subprocess.check_output(args, text=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    target = next(
        line.split(":", 1)[1].strip()
        for line in command("rustc", "-vV").splitlines()
        if line.startswith("host:")
    )
    metadata = json.loads(
        command("cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target)
    )
    tree = command(
        "cargo",
        "tree",
        "--locked",
        "--target",
        target,
        "-e",
        "normal,build",
        "--prefix",
        "none",
        "--format",
        "{p}",
    )
    selected = {
        (match.group(1), match.group(2))
        for line in tree.splitlines()
        if (match := re.match(r"^([^ ]+) v([^ ]+)", line))
    }
    packages = sorted(
        (
            package
            for package in metadata["packages"]
            if package["id"] != metadata["resolve"]["root"]
            and (package["name"], package["version"]) in selected
        ),
        key=lambda package: (package["name"], package["version"]),
    )

    supplement_root = root / "third-party" / "licenses"
    sources = json.loads((supplement_root / "sources.json").read_text(encoding="utf-8-sig"))
    for source in sources:
        path = supplement_root / source["package"] / source["file"]
        import hashlib

        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != source["sha256"]:
            raise SystemExit(f"Supplement license digest differs: {path}")

    egui_packages = {
        "ecolor",
        "eframe",
        "egui",
        "egui-winit",
        "egui_glow",
        "emath",
        "epaint",
        "epaint_default_fonts",
    }
    license_name = re.compile(
        r"^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE|COPYRIGHT|OFL|UFL)([._-]|$)", re.I
    )
    output = [
        "THIRD-PARTY NOTICES - fpdf Rust dependencies",
        f"Target: {target}; Cargo.lock and selected normal/build dependencies.",
        "Package SPDX expressions are recorded verbatim. Original license texts follow.",
        "PDFium, Node.js, AI packages, and the bundled font have separate notices in this package.",
        f"Dependency package count: {len(packages)}",
    ]
    for package in packages:
        package_root = Path(package["manifest_path"]).parent
        files = {
            path.relative_to(package_root).as_posix(): path
            for path in package_root.rglob("*")
            if path.is_file() and license_name.match(path.name)
        }
        if package.get("license_file"):
            path = package_root / package["license_file"]
            files[path.relative_to(package_root).as_posix()] = path
        supplement_name = f"{package['name']}-{package['version']}"
        if package["name"] in egui_packages and package["version"] == "0.33.3":
            supplement_name = "egui-0.33.3"
        for source in sources:
            if source["package"] == supplement_name:
                files[f"upstream/{source['file']}"] = (
                    supplement_root / source["package"] / source["file"]
                )
        if not files:
            raise SystemExit(f"No original license files found for {package['name']} {package['version']}")
        output.extend(
            [
                "",
                "=" * 70,
                f"Package: {package['name']} {package['version']}",
                f"SPDX: {package.get('license') or 'see included license'}",
                f"Repository: {package.get('repository') or 'not declared'}",
            ]
        )
        for label in sorted(files):
            output.extend(["", f"--- {label} ---"])
            if label.startswith("upstream/"):
                source = next(
                    item
                    for item in sources
                    if item["package"] == supplement_name and item["file"] == label[9:]
                )
                output.extend([f"Source: {source['url']}", f"SHA-256: {source['sha256']}"])
            output.append(files[label].read_text(encoding="utf-8", errors="replace"))

    Path(args.output).write_text("\n".join(output) + "\n", encoding="utf-8")
    print(f"Collected {len(packages)} Rust dependency packages: {args.output}")


if __name__ == "__main__":
    main()
