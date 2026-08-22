#!/usr/bin/env python3
"""Executable contract tests for sequential release artifact provenance."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "scripts" / "lib"
sys.path.insert(0, str(LIB))
import bong_server_provenance as provenance  # noqa: E402


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_manifest(
    path: Path,
    binary: Path,
    commit: str,
    profile: str = "release",
    digest: str | None = None,
) -> None:
    path.write_text(
        json.dumps(
            {
                "binary": str(binary),
                "commit": commit,
                "profile": profile,
                "sha256": digest if digest is not None else sha256(binary),
            }
        )
        + "\n",
        encoding="utf-8",
    )


def rejects(label: str, callback) -> None:
    try:
        callback()
    except (OSError, provenance.ProvenanceError):
        return
    raise AssertionError(f"{label} was accepted")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="bong-provenance-test-") as temp:
        temp_root = Path(temp)
        binary = temp_root / "release-server"
        binary.write_bytes(b"immutable server bytes\n")
        binary.chmod(0o700)
        manifest = temp_root / "manifest.json"
        current_commit = subprocess.check_output(
            ["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True
        ).strip()
        write_manifest(manifest, binary, current_commit)

        assert provenance.check(manifest, ROOT) == binary
        destination_a = temp_root / "run-a" / "bong-server-release"
        destination_b = temp_root / "run-b" / "bong-server-release"
        destination_a.parent.mkdir()
        destination_b.parent.mkdir()
        assert provenance.copy_run_owned(manifest, ROOT, destination_a) == destination_a
        assert provenance.copy_run_owned(manifest, ROOT, destination_b) == destination_b
        assert destination_a != destination_b
        assert destination_a.read_bytes() == binary.read_bytes()
        assert destination_b.read_bytes() == binary.read_bytes()
        assert sha256(destination_a) == sha256(binary)
        assert sha256(destination_b) == sha256(binary)
        assert destination_a.stat().st_mode & 0o777 == 0o700
        assert destination_b.stat().st_mode & 0o777 == 0o700
        assert destination_a != binary
        assert destination_b != binary
        assert destination_a != destination_b
        rejects(
            "unsafe relative destination",
            lambda: provenance.copy_run_owned(manifest, ROOT, Path("relative-server")),
        )
        unsafe_parent = temp_root / "unsafe-parent"
        unsafe_parent.mkdir()
        unsafe_link = temp_root / "unsafe-link"
        unsafe_link.symlink_to(unsafe_parent, target_is_directory=True)
        rejects(
            "symlink destination parent",
            lambda: provenance.copy_run_owned(
                manifest, ROOT, unsafe_link / "bong-server-release"
            ),
        )
        preexisting = temp_root / "preexisting-server"
        preexisting.write_bytes(b"caller-owned bytes\n")
        rejects("pre-existing destination", lambda: provenance.copy_run_owned(manifest, ROOT, preexisting))
        assert preexisting.read_bytes() == b"caller-owned bytes\n"

        stale = temp_root / "stale.json"
        write_manifest(stale, binary, "0" * 40)
        rejects("wrong commit", lambda: provenance.check(stale, ROOT))

        bad_digest = temp_root / "bad-digest.json"
        write_manifest(bad_digest, binary, current_commit, digest="0" * 64)
        rejects("producer digest mismatch", lambda: provenance.check(bad_digest, ROOT))

        debug_manifest = temp_root / "debug.json"
        write_manifest(debug_manifest, binary, current_commit, profile="debug")
        rejects("wrong profile", lambda: provenance.check(debug_manifest, ROOT))

        symlink = temp_root / "symlink-server"
        symlink.symlink_to(binary)
        symlink_manifest = temp_root / "symlink.json"
        write_manifest(symlink_manifest, symlink, current_commit)
        rejects("symlink artifact", lambda: provenance.check(symlink_manifest, ROOT))

        directory = temp_root / "not-a-file"
        directory.mkdir()
        non_file_manifest = temp_root / "non-file.json"
        non_file_manifest.write_text(
            json.dumps(
                {
                    "binary": str(directory),
                    "commit": current_commit,
                    "profile": "release",
                    "sha256": "0" * 64,
                }
            )
            + "\n",
            encoding="utf-8",
        )
        rejects("non-regular artifact", lambda: provenance.check(non_file_manifest, ROOT))

        binary_mutated = temp_root / "mutated-server"
        binary_mutated.write_bytes(b"original\n")
        mutation_manifest = temp_root / "mutation.json"
        write_manifest(mutation_manifest, binary_mutated, current_commit)
        binary_mutated.write_bytes(b"mutated producer\n")
        rejects("producer mutation", lambda: provenance.check(mutation_manifest, ROOT))

        copy_digest = temp_root / "copy-digest.json"
        write_manifest(copy_digest, binary, current_commit)
        tampered_destination = temp_root / "run-c" / "bong-server-release"
        tampered_destination.parent.mkdir()
        original_replace = provenance.os.replace

        def injected_replace(source, destination):
            original_replace(source, destination)
            Path(destination).write_bytes(b"tampered after atomic replace\n")

        provenance.os.replace = injected_replace
        try:
            rejects(
                "copy digest mismatch",
                lambda: provenance.copy_run_owned(copy_digest, ROOT, tampered_destination),
            )
        finally:
            provenance.os.replace = original_replace
        assert not tampered_destination.exists(), "failed post-replace validation must remove only this invocation's copy"

        workflow = (ROOT / ".github" / "workflows" / "e2e.yml").read_text(encoding="utf-8")
        assert workflow.count("cargo build --release") == 1
        assert workflow.count(
            "BONG_E2E_PREBUILT_SERVER_MANIFEST: ${{ needs.build-release.outputs.manifest }}"
        ) == 2
        assert workflow.count("id: server-release-artifact") == 1
        assert workflow.count("../scripts/lib/bong_server_provenance.py check") == 1
        assert workflow.count("bash scripts/bot-e2e.sh") == 2
        assert 'build_binary="${CARGO_TARGET_DIR:?}/release/bong-server"' in workflow
        assert 'test ! -L "$build_binary"' in workflow

        bot_script = (ROOT / "scripts" / "bot-e2e.sh").read_text(encoding="utf-8")
        prebuilt = bot_script.split(
            'if [ -n "$BONG_E2E_PREBUILT_SERVER_MANIFEST" ]; then', 1
        )[1].split("else", 1)[0]
        assert prebuilt.count("bong_server_provenance.py\" copy") == 1
        assert "cargo build" not in prebuilt
        assert 'SERVER_BINARY="$EVIDENCE_DIR/bong-server-release"' in prebuilt
        assert 'exec "$SERVER_BINARY"' in bot_script

    print("bong-server-provenance/workflow contract PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
