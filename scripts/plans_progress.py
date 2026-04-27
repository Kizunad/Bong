#!/usr/bin/env python3
"""Render Bong project plan progress section into README.md.

Reads docs/plans-progress.yaml and writes a progress block bounded by
BEGIN/END HTML comment markers in README.md. Idempotent: re-running with
unchanged input leaves README.md untouched.

Usage:
    python3 scripts/plans_progress.py            # update README.md in place
    python3 scripts/plans_progress.py --print    # print rendered block to stdout, no write
    python3 scripts/plans_progress.py --check    # exit non-zero if README.md is stale
"""

from __future__ import annotations

import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    sys.stderr.write("PyYAML required. Install with: pip install pyyaml\n")
    sys.exit(2)


REPO_ROOT = Path(__file__).resolve().parent.parent
DATA_FILE = REPO_ROOT / "docs" / "plans-progress.yaml"
README_FILE = REPO_ROOT / "README.md"

BEGIN_MARKER = "<!-- BEGIN:PLANS_PROGRESS -->"
END_MARKER = "<!-- END:PLANS_PROGRESS -->"

STATE_LABEL = {
    "merged": "merged",
    "active-implementing": "wip",
    "active-design": "design",
    "skeleton": "skeleton",
    "finished": "done",
}
STATE_ORDER = ["merged", "active-implementing", "active-design", "skeleton", "finished"]
STATE_DESC = {
    "merged": "代码已合并主线，plan 主体落地",
    "active-implementing": "设计 active，部分代码已落地，仍在推进",
    "active-design": "设计 active，零或近零代码",
    "skeleton": "骨架 plan，等待开工",
    "finished": "已归档（M0/M1 阶段产物）",
}

BAR_WIDTH = 12
TOTAL_BAR_WIDTH = 30


def progress_bar(pct: float, width: int = BAR_WIDTH) -> str:
    pct = max(0.0, min(100.0, pct))
    filled = int(round(pct / 100 * width))
    return "█" * filled + "░" * (width - filled)


def render(data: dict) -> str:
    plans = data["plans"]
    groups = data["groups"]

    lines: list[str] = []

    # ===== Overall =====
    n_plans = len(plans)
    overall = sum(p["percent"] for p in plans) / n_plans if n_plans else 0
    counts: dict[str, int] = {}
    for p in plans:
        counts[p["state"]] = counts.get(p["state"], 0) + 1

    lines.append("## Plan 进度")
    lines.append("")
    lines.append(f"_自动生成于 {data['generated_at']} · 共 {n_plans} 份 plan_")
    lines.append("")
    lines.append(f"```")
    lines.append(f"总进度  {progress_bar(overall, TOTAL_BAR_WIDTH)} {overall:5.1f}%")
    lines.append(f"```")
    lines.append("")

    # State counts inline
    state_parts = []
    for state in STATE_ORDER:
        n = counts.get(state, 0)
        if n:
            state_parts.append(f"`{STATE_LABEL[state]}` {n}")
    lines.append("**分布**：" + " · ".join(state_parts))
    lines.append("")

    # ===== Per-group =====
    for grp in groups:
        gid = grp["id"]
        grp_plans = [p for p in plans if p["group"] == gid]
        if not grp_plans:
            continue

        grp_plans.sort(key=lambda p: (-p["percent"], p["file"]))
        avg = sum(p["percent"] for p in grp_plans) / len(grp_plans)

        lines.append(f"### {grp['title']}")
        lines.append(
            f"_{grp['description']} · {len(grp_plans)} 份 · 组均 {avg:.0f}%_"
        )
        lines.append("")
        lines.append("| 状态 | Plan | 进度 | PR | 最近更新 |")
        lines.append("|---|---|---|---|---|")
        for p in grp_plans:
            label = STATE_LABEL[p["state"]]
            bar = progress_bar(p["percent"])
            pct = p["percent"]
            pr_refs = p.get("pr_refs") or []
            pr_str = " ".join(f"#{n}" for n in pr_refs) or "—"
            updated = p.get("last_updated") or "—"
            title = p["title"]
            file = p["file"]
            lines.append(
                f"| `{label}` | **{title}** <br/><sub>`{file}`</sub> "
                f"| `{bar}` {pct:3d}% | {pr_str} | {updated} |"
            )
        lines.append("")

    # ===== Legend =====
    lines.append("### 图例")
    lines.append("")
    for state in STATE_ORDER:
        if counts.get(state, 0) == 0:
            continue
        lines.append(f"- `{STATE_LABEL[state]}` — {STATE_DESC[state]}")
    lines.append("")
    lines.append(
        "_数据源：[`docs/plans-progress.yaml`](docs/plans-progress.yaml) · "
        "渲染脚本：[`scripts/plans_progress.py`](scripts/plans_progress.py) · "
        "经 GitHub Action 在 plan 改动时自动更新_"
    )

    return "\n".join(lines)


def build_block(rendered: str) -> str:
    return f"{BEGIN_MARKER}\n{rendered}\n{END_MARKER}"


def splice_readme(existing: str, block: str) -> str:
    if BEGIN_MARKER in existing and END_MARKER in existing:
        before, _, rest = existing.partition(BEGIN_MARKER)
        _, _, after = rest.partition(END_MARKER)
        return before + block + after
    return existing.rstrip() + "\n\n" + block + "\n"


DEFAULT_README_TEMPLATE = """# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust 无头 MC 服务器（Valence on Bevy 0.14 ECS，MC 1.20.1 协议 763）
- **client/** — Fabric 1.20.1 微端（Java 17，owo-lib UI）
- **agent/** — LLM "天道" agent 层（TypeScript，三 Agent 并发推演）
- **worldgen/** — Python 地形生成流水线
- **library-web/** — 末法残土图书馆前端（Astro）

详见 [`CLAUDE.md`](CLAUDE.md)。

{block}
"""


def main(argv: list[str]) -> int:
    if not DATA_FILE.exists():
        sys.stderr.write(f"ERROR: {DATA_FILE} not found\n")
        return 2

    with DATA_FILE.open(encoding="utf-8") as f:
        data = yaml.safe_load(f)

    rendered = render(data)
    block = build_block(rendered)

    if "--print" in argv:
        print(rendered)
        return 0

    if README_FILE.exists():
        existing = README_FILE.read_text(encoding="utf-8")
        new_content = splice_readme(existing, block)
    else:
        existing = ""
        new_content = DEFAULT_README_TEMPLATE.format(block=block)

    if "--check" in argv:
        if new_content != existing:
            sys.stderr.write(
                f"README.md is stale. Run: python3 {Path(__file__).relative_to(REPO_ROOT)}\n"
            )
            return 1
        print("README.md up to date.")
        return 0

    if new_content != existing:
        README_FILE.write_text(new_content, encoding="utf-8")
        print(f"✓ {README_FILE.relative_to(REPO_ROOT)} updated")
    else:
        print(f"✓ {README_FILE.relative_to(REPO_ROOT)} unchanged")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
