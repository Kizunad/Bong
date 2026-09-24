#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import stat
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECK = ROOT / "scripts/check-proto-breaking.sh"
ALLOWLIST = ROOT / "proto/buf-breaking-approvals.tsv"


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

    def _run(
        self,
        *,
        proto_kind: str,
        base_ref: str = "base",
        advance_remote_base: bool = False,
        buf_output: str = "",
        buf_exit: int = 0,
        allowlist_text: str | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], str, pathlib.Path]:
        sha = self._commit_base(proto_kind=proto_kind)
        if advance_remote_base:
            # 让远端 base 前进一个提交：checkout 里已有的 origin/base 追踪
            # ref 随即落后，脚本 fetch 若不带 '+' 前缀会被 git 以
            # non-fast-forward 拒绝（持久 runner / fetch-depth 0 场景）。
            (self.seed / "README").write_text("advance\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(self.seed), "add", "README"], check=True)
            subprocess.run(
                ["git", "-C", str(self.seed), "commit", "-m", "advance"],
                check=True,
                capture_output=True,
            )
            sha = subprocess.run(
                ["git", "-C", str(self.seed), "rev-parse", "HEAD"],
                check=True,
                text=True,
                capture_output=True,
            ).stdout.strip()
            subprocess.run(
                ["git", "-C", str(self.seed), "push", "origin", "HEAD:base"],
                check=True,
                capture_output=True,
            )
        scripts = self.checkout / "scripts"
        scripts.mkdir()
        local_check = scripts / CHECK.name
        local_check.write_bytes(CHECK.read_bytes())
        local_check.chmod(local_check.stat().st_mode | stat.S_IXUSR)
        if proto_kind == "tree":
            (self.checkout / "proto").mkdir(exist_ok=True)
            allowlist_path = self.checkout / "proto" / ALLOWLIST.name
            allowlist_path.write_bytes(ALLOWLIST.read_bytes())
            if allowlist_text is not None:
                allowlist_path.write_text(allowlist_text, encoding="utf-8")
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        buf_log = self.root / "buf.log"
        fake_buf = bin_dir / "buf"
        fake_buf.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            "printf '%s\\n' \"$*\" >\"$BUF_LOG\"\n"
            "if [ -n \"${BUF_OUTPUT:-}\" ]; then printf '%s\\n' \"$BUF_OUTPUT\"; fi\n"
            "exit \"${BUF_EXIT:-0}\"\n",
            encoding="utf-8",
        )
        fake_buf.chmod(0o700)
        env = os.environ.copy()
        env.update(
            {
                "BASE_REF": base_ref,
                "BUF_LOG": str(buf_log),
                "BUF_OUTPUT": buf_output,
                "BUF_EXIT": str(buf_exit),
                "PATH": f"{bin_dir}:{env['PATH']}",
            }
        )
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
            f"breaking --error-format=json --against ../.git#ref={sha},subdir=proto",
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

    def test_missing_base_ref_fails_closed_not_first_pr_skip(self) -> None:
        # The remote only ever receives the real "base" branch, so requesting a
        # ref the remote cannot resolve must fail the step — a fetch failure is
        # a verification-environment error, never the "first PR without proto/"
        # skip path (plan-bughunt-proto-breaking-check-shallow-skip-v1 TODO 3).
        result, _sha, buf_log = self._run(proto_kind="tree", base_ref="__definitely_missing_base_ref__")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("skipping breaking check (first PR)", result.stdout)
        self.assertFalse(buf_log.exists())

    def test_stale_local_base_ref_is_force_updated(self) -> None:
        # 持久 runner / fetch-depth 0 的 checkout 里 refs/remotes/origin/base
        # 已存在但落后于远端。脚本的 fetch 必须能强制更新该 ref（'+' 前缀），
        # 否则 git 拒绝 non-fast-forward 更新，set -euo pipefail 下整个 step 失败。
        # buf 参数必须指向推进后的新 base commit —— 证明 fetch 真的刷新了陈旧 ref。
        result, sha, buf_log = self._run(proto_kind="tree", advance_remote_base=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("non-fast-forward", result.stderr)
        self.assertEqual(
            buf_log.read_text(encoding="utf-8").strip(),
            f"breaking --error-format=json --against ../.git#ref={sha},subdir=proto",
        )

    def test_approved_breaking_findings_are_allowed_exactly(self) -> None:
        message = 'Previously present message "ApprovedMessage" was deleted from file.'
        fingerprint = hashlib.sha256(
            "\0".join(("MESSAGE_NO_DELETE", "bong/envelope.proto", message)).encode("utf-8")
        ).hexdigest()
        findings = json.dumps(
            {"type": "MESSAGE_NO_DELETE", "path": "bong/envelope.proto", "message": message}
        )
        allowlist = f"MESSAGE_NO_DELETE\tbong/envelope.proto\t{fingerprint}\ttest approval\n"
        result, _sha, _buf_log = self._run(
            proto_kind="tree", buf_output=findings, buf_exit=100, allowlist_text=allowlist
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("已批准协议删除", result.stdout)

    def test_unapproved_breaking_finding_still_fails_closed(self) -> None:
        finding = json.dumps(
            {
                "type": "MESSAGE_NO_DELETE",
                "path": "bong/envelope.proto",
                "message": 'Previously present message "AnotherMessage" was deleted from file.',
            }
        )
        result, _sha, _buf_log = self._run(proto_kind="tree", buf_output=finding, buf_exit=100)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("未批准的 proto breaking", result.stderr)

    def test_ci_invokes_the_executable_contract(self) -> None:
        workflow = (ROOT / ".github/workflows/e2e.yml").read_text(encoding="utf-8")
        self.assertIn("bash scripts/check-proto-breaking.sh", workflow)
        self.assertIn("python3 scripts/tests/proto_breaking_base_ref_contract_test.py", workflow)


if __name__ == "__main__":
    unittest.main()
