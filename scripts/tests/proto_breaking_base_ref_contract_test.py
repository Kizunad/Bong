#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import stat
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECK = ROOT / "scripts/check-proto-breaking.sh"


class ProtoBreakingBaseRefContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        self.remote = self.root / "remote.git"
        self.seed = self.root / "seed"
        self.checkout = self.root / "checkout"
        subprocess.run(["git", "init", "--bare", str(self.remote)], check=True, capture_output=True)
        subprocess.run(["git", "init", "-b", "main", str(self.seed)], check=True, capture_output=True)
        subprocess.run(["git", "-C", str(self.seed), "config", "user.name", "test"], check=True)
        subprocess.run(["git", "-C", str(self.seed), "config", "user.email", "test@example.com"], check=True)
        subprocess.run(["git", "-C", str(self.seed), "remote", "add", "origin", str(self.remote)], check=True)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _commit_base(self, *, proto_kind: str) -> str:
        if proto_kind == "tree":
            (self.seed / "proto").mkdir()
            (self.seed / "proto/buf.yaml").write_text("version: v2\n", encoding="utf-8")
        elif proto_kind == "blob":
            (self.seed / "proto").write_text("not a directory\n", encoding="utf-8")
        elif proto_kind != "absent":
            raise AssertionError(proto_kind)
        (self.seed / "README").write_text(proto_kind, encoding="utf-8")
        subprocess.run(["git", "-C", str(self.seed), "add", "README"], check=True)
        if proto_kind != "absent":
            subprocess.run(["git", "-C", str(self.seed), "add", "proto"], check=True)
        subprocess.run(["git", "-C", str(self.seed), "commit", "-m", proto_kind], check=True, capture_output=True)
        sha = subprocess.run(
            ["git", "-C", str(self.seed), "rev-parse", "HEAD"],
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip()
        subprocess.run(["git", "-C", str(self.seed), "push", "origin", "HEAD:base"], check=True, capture_output=True)
        subprocess.run(
            ["git", "clone", "--branch", "base", str(self.remote), str(self.checkout)],
            check=True,
            capture_output=True,
        )
        return sha

    def _run(self, *, proto_kind: str) -> tuple[subprocess.CompletedProcess[str], str, pathlib.Path]:
        sha = self._commit_base(proto_kind=proto_kind)
        scripts = self.checkout / "scripts"
        scripts.mkdir()
        local_check = scripts / CHECK.name
        local_check.write_bytes(CHECK.read_bytes())
        local_check.chmod(local_check.stat().st_mode | stat.S_IXUSR)
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        buf_log = self.root / "buf.log"
        fake_buf = bin_dir / "buf"
        fake_buf.write_text(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$*\" >\"$BUF_LOG\"\n",
            encoding="utf-8",
        )
        fake_buf.chmod(0o700)
        env = os.environ.copy()
        env.update({"BASE_REF": "base", "BUF_LOG": str(buf_log), "PATH": f"{bin_dir}:{env['PATH']}"})
        result = subprocess.run(
            ["bash", str(local_check)],
            cwd=self.checkout,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        return result, sha, buf_log

    def test_tree_runs_buf_against_verified_commit(self) -> None:
        result, sha, buf_log = self._run(proto_kind="tree")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            buf_log.read_text(encoding="utf-8").strip(),
            f"breaking --against ../.git#ref={sha},subdir=proto",
        )

    def test_absent_proto_skips_first_pr_only(self) -> None:
        result, sha, buf_log = self._run(proto_kind="absent")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"proto/ not found on verified base commit {sha}", result.stdout)
        self.assertFalse(buf_log.exists())

    def test_non_tree_proto_fails_closed(self) -> None:
        result, _sha, buf_log = self._run(proto_kind="blob")
        self.assertEqual(result.returncode, 1)
        self.assertIn("unexpected git object type: blob", result.stderr)
        self.assertFalse(buf_log.exists())

    def test_ci_invokes_the_executable_contract(self) -> None:
        workflow = (ROOT / ".github/workflows/e2e.yml").read_text(encoding="utf-8")
        self.assertIn("bash scripts/check-proto-breaking.sh", workflow)
        self.assertIn("python3 scripts/tests/proto_breaking_base_ref_contract_test.py", workflow)


if __name__ == "__main__":
    unittest.main()
