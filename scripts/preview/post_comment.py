#!/usr/bin/env python3
"""post_comment.py — 在 PR 上发/编辑 worldgen-snapshot 评论。

plan-worldgen-snapshot-v1 §3.2。

用法（需要 GH_TOKEN env）:
  python3 scripts/preview/post_comment.py \\
    --pr <PR#> \\
    --repo <owner>/<repo> \\
    --artifact-name worldgen-snapshot-<PR#> \\
    --run-id <gh actions run id> \\
    --commit <head sha>

防刷:首行带 marker `[bong-snapshot]`,已存在该 marker 的 PR 评论改为 edit。

不依赖第三方:用 stdlib urllib + json + os。
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

MARKER = "[bong-snapshot]"


def _gh_request(
    method: str, url: str, token: str, body: dict | None = None
) -> tuple[int, dict | list | None]:
    """简易 GitHub API 调用。返回 (status, parsed json)."""
    data: bytes | None = None
    if body is not None:
        data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req) as resp:
            payload = resp.read().decode("utf-8")
            return resp.status, (json.loads(payload) if payload else None)
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode("utf-8"))
        except Exception:
            return e.code, None


def find_existing_marker_comment(repo: str, pr: int, token: str) -> int | None:
    """搜 PR 已有 [bong-snapshot] 首行评论。返回 comment id 或 None."""
    page = 1
    while True:
        url = f"https://api.github.com/repos/{repo}/issues/{pr}/comments?per_page=100&page={page}"
        status, data = _gh_request("GET", url, token)
        if status != 200 or not isinstance(data, list) or not data:
            return None
        for c in data:
            body = (c.get("body") or "").strip()
            if body.startswith(MARKER):
                return c.get("id")
        if len(data) < 100:
            return None
        page += 1


def build_body(repo: str, pr: int, run_id: int, commit: str, artifact_name: str) -> str:
    """生成 markdown 评论。

    artifact 下载 URL 格式:
      https://github.com/<repo>/actions/runs/<run_id>/artifacts/<artifact_id>
    但我们不知道 artifact_id（要先查 API）。简化:贴 run URL,用户从 run 页面下载。
    """
    short = commit[:8] if commit else "?"
    run_url = f"https://github.com/{repo}/actions/runs/{run_id}"
    return (
        f"{MARKER} commit `{short}`\n"
        f"\n"
        f"## Worldgen 快照（plan-worldgen-snapshot-v1 P0+P1）\n"
        f"\n"
        f"**双轨快照**：\n"
        f"- 🎮 **client 真画面截图**（5 角度，spawn 周围 ±400 blocks 范围 view distance 32 chunks）：\n"
        f"  - top 全俯视 / iso_ne / iso_nw / iso_se / iso_sw\n"
        f"- 🗺️ **raster 顶视全图**（worldgen pipeline 输出，几千 blocks 大尺度）：\n"
        f"  - focus-layout / focus-surface / focus-height / 9 个 zone × 3 通道\n"
        f"\n"
        f"**装饰**：spawn 中心 (8, 80, 8) 木牌「初醒原 / 灵气 0.3 / 危险 1 / 末法 spawn」 + 12 格 end_rod 灵脉柱（client iso 视角内可见）\n"
        f"\n"
        f"### 下载\n"
        f"\n"
        f"📦 [Artifact `{artifact_name}` 下载（CI run 页面）]({run_url})\n"
        f"\n"
        f"内含 `preview-grid.png`（5 角度 + 2 raster 拼图总览）以及所有原图。\n"
        f"\n"
        f"---\n"
        f"\n"
        f"_自动评论 · marker `{MARKER}` · 同 PR 后续 push 会编辑本评论而非新发_"
    )


def post_or_edit(
    repo: str, pr: int, run_id: int, commit: str, artifact_name: str, token: str
) -> int:
    """主入口。返回 exit code (0 ok)."""
    body = build_body(repo, pr, run_id, commit, artifact_name)
    existing = find_existing_marker_comment(repo, pr, token)
    if existing is not None:
        url = f"https://api.github.com/repos/{repo}/issues/comments/{existing}"
        status, _ = _gh_request("PATCH", url, token, {"body": body})
        if status >= 300:
            print(f"[post_comment] edit comment 失败 status={status}", file=sys.stderr)
            return 1
        print(f"[post_comment] edited existing comment id={existing}")
    else:
        url = f"https://api.github.com/repos/{repo}/issues/{pr}/comments"
        status, _ = _gh_request("POST", url, token, {"body": body})
        if status >= 300:
            print(f"[post_comment] post comment 失败 status={status}", file=sys.stderr)
            return 1
        print("[post_comment] posted new comment")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="post_comment.py")
    parser.add_argument("--repo", required=True, help="owner/repo")
    parser.add_argument("--pr", required=True, type=int)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--commit", required=True)
    parser.add_argument(
        "--artifact-name",
        required=True,
        help="artifact name 用于评论中显示，如 worldgen-snapshot-71",
    )
    args = parser.parse_args()

    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        print("[post_comment] 错误: GH_TOKEN 或 GITHUB_TOKEN env 未设", file=sys.stderr)
        return 2

    return post_or_edit(args.repo, args.pr, args.run_id, args.commit, args.artifact_name, token)


if __name__ == "__main__":
    sys.exit(main())
