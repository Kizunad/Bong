#!/usr/bin/env python3
"""Validate and copy the CI's provenance-pinned Bong server artifact.

The release artifact is built once by CI and consumed by sequential bot stages.
Each consumer gets an independent run-owned copy; the shared source is never
executed directly and every copy is checked against the manifest digest.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

_SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")


class ProvenanceError(ValueError):
    """Raised when a manifest or artifact fails closed validation."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _current_head(root: Path) -> str:
    try:
        return subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise ProvenanceError(f"cannot read current checkout HEAD: {error}") from error


def _load_and_validate(manifest_path: Path, root: Path) -> tuple[Path, str]:
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise ProvenanceError(f"manifest is not a regular file: {manifest_path}")
    try:
        data = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError) as error:
        raise ProvenanceError(f"invalid provenance manifest: {error}") from error
    if not isinstance(data, dict):
        raise ProvenanceError("provenance manifest must be a JSON object")

    binary_value = data.get("binary")
    expected_commit = data.get("commit")
    expected_profile = data.get("profile")
    expected_sha = data.get("sha256")
    if not all(
        isinstance(value, str)
        for value in (binary_value, expected_commit, expected_profile, expected_sha)
    ):
        raise ProvenanceError("provenance manifest fields must all be strings")
    if expected_profile != "release":
        raise ProvenanceError(f"prebuilt server profile is not release: {expected_profile}")
    if not _SHA256_RE.fullmatch(expected_sha):
        raise ProvenanceError("provenance manifest sha256 must be 64 hexadecimal characters")

    binary = Path(binary_value)
    if not binary.is_absolute() or binary.is_symlink() or not binary.is_file():
        raise ProvenanceError(f"prebuilt server is not a real file: {binary}")
    if not os.access(binary, os.R_OK):
        raise ProvenanceError(f"prebuilt server is not readable: {binary}")

    current_commit = _current_head(root.resolve())
    if expected_commit != current_commit:
        raise ProvenanceError(
            f"prebuilt server checkout mismatch: manifest={expected_commit} current={current_commit}"
        )

    actual_sha = _sha256(binary)
    if expected_sha.lower() != actual_sha:
        raise ProvenanceError(
            f"prebuilt server sha256 mismatch: manifest={expected_sha} actual={actual_sha}"
        )
    return binary, actual_sha


def check(manifest_path: Path, root: Path) -> Path:
    binary, _ = _load_and_validate(manifest_path, root)
    return binary


def copy_run_owned(manifest_path: Path, root: Path, destination: Path) -> Path:
    source, expected_sha = _load_and_validate(manifest_path, root)
    if not destination.is_absolute():
        raise ProvenanceError(f"run-owned destination must be absolute: {destination}")
    if destination == destination.parent or destination == Path("/"):
        raise ProvenanceError(f"run-owned destination cannot be a root directory: {destination}")
    if destination.is_symlink() or destination.exists():
        raise ProvenanceError(
            f"run-owned destination must not pre-exist or be a symlink: {destination}"
        )
    if not destination.parent.is_dir() or destination.parent.is_symlink():
        raise ProvenanceError(
            f"run-owned destination parent is not a real directory: {destination.parent}"
        )
    destination_parent = destination.parent.resolve(strict=True)
    if destination.parent != destination_parent:
        raise ProvenanceError(
            f"run-owned destination parent is not canonical: {destination.parent}"
        )

    temporary = Path(f"{destination}.tmp")
    if temporary.is_symlink() or temporary.exists():
        raise ProvenanceError(f"run-owned temporary destination already exists or is unsafe: {temporary}")

    installed_destination = False
    try:
        with source.open("rb") as source_stream, temporary.open("xb") as destination_stream:
            shutil.copyfileobj(source_stream, destination_stream, length=1024 * 1024)
            destination_stream.flush()
            os.fsync(destination_stream.fileno())
        temporary.chmod(0o700)
        copied_sha = _sha256(temporary)
        if copied_sha != expected_sha:
            raise ProvenanceError(
                f"copied prebuilt server sha256 mismatch: manifest={expected_sha} actual={copied_sha}"
            )
        os.replace(temporary, destination)
        installed_destination = True
        if destination.is_symlink() or not destination.is_file():
            raise ProvenanceError(f"run-owned copy is not a regular file: {destination}")
        if _sha256(destination) != expected_sha:
            raise ProvenanceError(f"run-owned copy failed final digest check: {destination}")
        destination.chmod(0o700)
    except Exception:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
        if installed_destination and destination.is_file() and not destination.is_symlink():
            destination.unlink()
        raise
    return destination


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("manifest", type=Path)
    check_parser.add_argument("root", type=Path)

    copy_parser = subparsers.add_parser("copy")
    copy_parser.add_argument("manifest", type=Path)
    copy_parser.add_argument("root", type=Path)
    copy_parser.add_argument("destination", type=Path)

    args = parser.parse_args(argv)
    try:
        if args.command == "check":
            print(check(args.manifest, args.root))
        else:
            print(copy_run_owned(args.manifest, args.root, args.destination))
    except (OSError, ProvenanceError) as error:
        print(f"[bong-provenance] {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
