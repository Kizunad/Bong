#!/usr/bin/env python3
"""
plan_overview_gen.py
--------------------
Read skeleton plan markdown files and generate a dark-themed HTML overview
page for each one, suitable for reviewing readiness to promote skeleton -> active.
"""

import re
import os
from pathlib import Path

SKELETON_DIR = Path(__file__).resolve().parent.parent.parent / "docs" / "plans-skeleton"
OUTPUT_DIR = Path(__file__).resolve().parent

PLAN_FILES = [
    "plan-dugu-v2.md",
    "plan-tuike-v2.md",
    "plan-baomai-v3.md",
    "plan-zhenfa-v2.md",
    "plan-tsy-raceout-v2.md",
    "plan-poison-trait-v1.md",
]


# ---------------------------------------------------------------------------
# Parsing helpers
# ---------------------------------------------------------------------------

def _first_heading(text: str) -> str:
    m = re.search(r"^#\s+(.+)$", text, re.MULTILINE)
    return m.group(1).strip() if m else "(untitled)"


def _first_paragraph(text: str) -> str:
    """First non-blank paragraph after the first heading."""
    lines = text.split("\n")
    started = False
    buf = []
    for line in lines:
        if not started:
            if line.startswith("# "):
                started = True
            continue
        stripped = line.strip()
        if not stripped:
            if buf:
                break
            continue
        if stripped.startswith("#") or stripped.startswith("---"):
            if buf:
                break
            continue
        buf.append(stripped)
    return " ".join(buf) if buf else ""


def _extract_section(text: str, heading_pattern: str) -> str:
    """Extract everything under a heading matching *heading_pattern* until the
    next heading of equal or higher level (or EOF)."""
    pat = re.compile(
        r"^(#{1,4})\s+" + heading_pattern + r".*$", re.MULTILINE | re.IGNORECASE
    )
    m = pat.search(text)
    if not m:
        return ""
    level = len(m.group(1))
    start = m.end()
    # find next heading of same or higher level
    next_h = re.compile(r"^#{1," + str(level) + r"}\s+", re.MULTILINE)
    m2 = next_h.search(text, start)
    return text[start : m2.start() if m2 else len(text)].strip()


def _extract_worldview_anchors(text: str) -> str:
    for line in text.split("\n"):
        if re.match(r"\*\*世界观锚点\*\*", line.strip()):
            return line.strip().replace("**世界观锚点**：", "").replace("**世界观锚点**:", "")
    return ""


def _extract_dependencies(text: str) -> list[dict]:
    """Parse '**前置依赖**:' block into list of {name, status}."""
    deps = []
    in_deps = False
    for line in text.split("\n"):
        stripped = line.strip()
        if "前置依赖" in stripped and "**" in stripped:
            in_deps = True
            continue
        if in_deps:
            if stripped.startswith("**") and "前置" not in stripped:
                break
            if stripped.startswith("- "):
                status = "gray"
                if "✅" in stripped:  # checkmark
                    status = "green"
                elif "⏳" in stripped:  # hourglass
                    status = "amber"
                elif "\U0001f195" in stripped:  # NEW
                    status = "blue"
                elif "\U0001f7e1" in stripped:  # yellow circle
                    status = "amber"
                name_m = re.search(r"`([^`]+)`", stripped)
                name = name_m.group(1) if name_m else stripped[2:60]
                deps.append({"name": name, "status": status, "raw": stripped[2:]})
    return deps


def _extract_reverse_deps(text: str) -> list[dict]:
    deps = []
    in_rev = False
    for line in text.split("\n"):
        stripped = line.strip()
        if "反向被依赖" in stripped and "**" in stripped:
            in_rev = True
            continue
        if in_rev:
            if stripped.startswith("**") and "反向" not in stripped:
                break
            if stripped.startswith("---"):
                break
            if stripped.startswith("- "):
                status = "gray"
                if "✅" in stripped:
                    status = "green"
                elif "⏳" in stripped:
                    status = "amber"
                elif "\U0001f195" in stripped:
                    status = "blue"
                name_m = re.search(r"`([^`]+)`", stripped)
                name = name_m.group(1) if name_m else stripped[2:60]
                deps.append({"name": name, "status": status, "raw": stripped[2:]})
    return deps


def _extract_phase_table(text: str) -> list[dict]:
    """Extract phase overview table from section 1 or subsection headings."""
    phases = []
    # Strategy 1: table rows with P0/P1/... pattern (most plans)
    for m in re.finditer(
        r"\|\s*\*\*(P\d+)\*\*\s*([⬜✅⏳\U0001f504⚠️]*)\s*\|([^|]+)\|([^|]+)\|",
        text,
    ):
        phase_id = m.group(1).strip()
        status_raw = m.group(2).strip()
        content = m.group(3).strip()
        acceptance = m.group(4).strip()
        status = "gray"
        if "✅" in status_raw:
            status = "green"
        elif "⏳" in status_raw:
            status = "amber"
        phases.append(
            {
                "id": phase_id,
                "status": status,
                "content": content[:120] + ("..." if len(content) > 120 else ""),
                "acceptance": acceptance[:100] + ("..." if len(acceptance) > 100 else ""),
            }
        )

    # Strategy 2: subsection headings like ### P0 --- Title (zhenfa style)
    if not phases:
        for m in re.finditer(
            r"^###\s+(P\d+)\s*[—\-]+\s*(.+)$", text, re.MULTILINE
        ):
            phase_id = m.group(1).strip()
            content = m.group(2).strip()
            phases.append(
                {
                    "id": phase_id,
                    "status": "gray",
                    "content": content[:120],
                    "acceptance": "",
                }
            )

    return phases


def _extract_skills(text: str) -> list[dict]:
    """Extract skill/move cards from section 2 subheadings."""
    skills = []
    # find ### headings that look like skill names (numbered or named)
    skill_headings = list(
        re.finditer(
            r"^###\s+(.+)$", text, re.MULTILINE
        )
    )
    for i, m in enumerate(skill_headings):
        heading = m.group(1).strip()
        # get block until next ### or ## or EOF
        start = m.end()
        if i + 1 < len(skill_headings):
            end = skill_headings[i + 1].start()
        else:
            # find next ## heading
            next_sec = re.search(r"^##\s+", text[start:], re.MULTILINE)
            end = start + next_sec.start() if next_sec else len(text)
        block = text[start:end]

        # extract qi cost
        qi_m = re.search(r"(?:qi|真元)\s*(?:消耗|cost)[^\d]*(\d[\d.+*/×\s（）()a-zA-Z_]*)", block, re.IGNORECASE)
        qi_cost = qi_m.group(1).strip()[:40] if qi_m else "-"

        # extract cooldown
        cd_m = re.search(r"(?:冷却|cooldown|cast\s*time)[^\d]*(\d[\d.smin\-→×/\s（）()a-zA-Z_]*)", block, re.IGNORECASE)
        cooldown = cd_m.group(1).strip()[:30] if cd_m else "-"

        # extract key mechanic from first **用途** or first sentence
        usage_m = re.search(r"\*\*用途\*\*[：:](.+?)(?:\n|$)", block)
        if usage_m:
            mechanic = usage_m.group(1).strip()[:100]
        else:
            # first meaningful line
            for line in block.split("\n"):
                s = line.strip()
                if s and not s.startswith("|") and not s.startswith("```"):
                    mechanic = s[:100]
                    break
            else:
                mechanic = "-"

        skills.append(
            {
                "name": heading,
                "qi_cost": qi_cost,
                "cooldown": cooldown,
                "mechanic": mechanic,
            }
        )
    return skills


def _extract_open_questions(text: str) -> list[str]:
    """Extract open question titles from section 5 / 6."""
    questions = []
    # pattern 1: ### #1 Title
    for m in re.finditer(r"^###\s+(#\d+.+)$", text, re.MULTILINE):
        questions.append(m.group(1).strip())
    # pattern 2: - [ ] **Q1 Title** (bold wraps entire Q+title)
    if not questions:
        for m in re.finditer(r"^\s*-\s+\[\s*\]\s+\*\*(Q[\w-]*\d+.+?)\*\*", text, re.MULTILINE):
            questions.append(m.group(1).strip())
    # pattern 3: - [ ] **Q1** Title (bold wraps only Q-number, title follows)
    if not questions:
        for m in re.finditer(r"^\s*-\s+\[\s*\]\s+\*\*(Q[\w-]*\d+)\*\*\s*(.+?)(?:\s*$)", text, re.MULTILINE):
            q_id = m.group(1).strip()
            q_rest = m.group(2).strip()
            # truncate at arrow or long text
            q_rest = q_rest.split("→")[0].strip()[:80]
            questions.append(f"{q_id} {q_rest}")
    return questions


def _extract_test_target(text: str) -> str:
    """Extract test count target."""
    m = re.search(r"下限\s*\*\*(\d+)\s*单测\*\*", text)
    if m:
        return m.group(1)
    m = re.search(r"≥\s*(\d+)", text)
    return m.group(1) if m else "-"


# ---------------------------------------------------------------------------
# HTML generation
# ---------------------------------------------------------------------------

def _status_badge(status: str) -> str:
    colors = {
        "green": ("#1a3a1a", "#4ade80", "✅"),
        "amber": ("#3a2a0a", "#fbbf24", "⏳"),
        "blue": ("#0a1a3a", "#60a5fa", "\U0001f195"),
        "gray": ("#1a1a1a", "#6b7280", "⬜"),
    }
    bg, fg, icon = colors.get(status, colors["gray"])
    return (
        f'<span style="display:inline-block;padding:2px 8px;border-radius:4px;'
        f"background:{bg};color:{fg};font-size:0.85em;margin-right:4px;\">"
        f"{icon}</span>"
    )


def _esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def generate_html(plan_data: dict) -> str:
    d = plan_data

    # dependency rows
    dep_rows = ""
    for dep in d["dependencies"]:
        badge = _status_badge(dep["status"])
        dep_rows += f'<tr><td>{badge} <code>{_esc(dep["name"])}</code></td></tr>\n'

    # reverse dep rows
    rev_rows = ""
    for dep in d["reverse_deps"]:
        badge = _status_badge(dep["status"])
        rev_rows += f'<tr><td>{badge} <code>{_esc(dep["name"])}</code></td></tr>\n'

    # phase rows
    phase_rows = ""
    for p in d["phases"]:
        badge = _status_badge(p["status"])
        phase_rows += (
            f'<tr><td style="white-space:nowrap;">{badge} <strong>{_esc(p["id"])}</strong></td>'
            f'<td>{_esc(p["content"])}</td>'
            f'<td style="color:#888;font-size:0.85em;">{_esc(p["acceptance"])}</td></tr>\n'
        )

    # skill cards
    skill_cards = ""
    for sk in d["skills"]:
        skill_cards += f"""
        <div class="skill-card">
          <div class="skill-name">{_esc(sk["name"])}</div>
          <div class="skill-row"><span class="label">真元消耗</span> <span class="val">{_esc(sk["qi_cost"])}</span></div>
          <div class="skill-row"><span class="label">冷却</span> <span class="val">{_esc(sk["cooldown"])}</span></div>
          <div class="skill-row"><span class="label">核心机制</span> <span class="val">{_esc(sk["mechanic"])}</span></div>
        </div>
        """

    # open questions
    oq_items = ""
    for q in d["open_questions"]:
        oq_items += f"<li>{_esc(q)}</li>\n"

    # worldview anchors - split on centered dot for readability
    wv_text = d["worldview_anchors"]
    wv_parts = re.split(r"\s*[·]\s*", wv_text)
    wv_items = "".join(f"<li>{_esc(p.strip())}</li>" for p in wv_parts if p.strip())

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>{_esc(d["title"])} - Plan Overview</title>
<style>
  :root {{
    --bg: #0a0a0a;
    --card-bg: #111111;
    --border: #222222;
    --gold: #c9a96e;
    --gold-dim: #8a7a4e;
    --text: #d4d4d4;
    --text-dim: #777;
    --green: #4ade80;
    --amber: #fbbf24;
    --blue: #60a5fa;
    --red: #f87171;
  }}
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    background: var(--bg);
    color: var(--text);
    font-family: "JetBrains Mono", "Fira Code", "Cascadia Code", monospace;
    font-size: 14px;
    line-height: 1.6;
    padding: 24px 32px;
    max-width: 1100px;
    margin: 0 auto;
  }}
  h1 {{
    color: var(--gold);
    font-size: 1.6em;
    border-bottom: 2px solid var(--gold-dim);
    padding-bottom: 8px;
    margin-bottom: 6px;
  }}
  .subtitle {{
    color: var(--text-dim);
    font-size: 0.9em;
    margin-bottom: 24px;
    line-height: 1.5;
  }}
  h2 {{
    color: var(--gold);
    font-size: 1.15em;
    margin-top: 28px;
    margin-bottom: 10px;
    padding-left: 8px;
    border-left: 3px solid var(--gold-dim);
  }}
  .section {{
    background: var(--card-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 14px 18px;
    margin-bottom: 16px;
  }}
  table {{ width: 100%; border-collapse: collapse; }}
  td, th {{
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }}
  th {{ color: var(--gold-dim); font-weight: 600; font-size: 0.85em; text-transform: uppercase; }}
  code {{
    background: #1a1a2e;
    color: var(--gold);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.92em;
  }}
  ul {{ padding-left: 18px; }}
  li {{ margin-bottom: 4px; color: var(--text-dim); font-size: 0.9em; }}
  .skill-grid {{
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }}
  .skill-card {{
    background: #0d0d15;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px 14px;
  }}
  .skill-name {{
    color: var(--gold);
    font-weight: 700;
    font-size: 1.0em;
    margin-bottom: 6px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 4px;
  }}
  .skill-row {{
    display: flex;
    gap: 8px;
    margin-bottom: 3px;
    font-size: 0.88em;
  }}
  .skill-row .label {{
    color: var(--gold-dim);
    min-width: 70px;
    flex-shrink: 0;
  }}
  .skill-row .val {{ color: var(--text); }}
  .stat-box {{
    display: inline-block;
    background: #0d0d15;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 14px;
    margin-right: 12px;
    margin-bottom: 8px;
  }}
  .stat-box .stat-label {{ color: var(--gold-dim); font-size: 0.8em; }}
  .stat-box .stat-val {{ color: var(--gold); font-size: 1.3em; font-weight: 700; }}
  .dep-grid {{
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 24px;
  }}
  .footer {{
    text-align: center;
    color: var(--text-dim);
    font-size: 0.75em;
    margin-top: 32px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }}
</style>
</head>
<body>

<h1>{_esc(d["title"])}</h1>
<div class="subtitle">{_esc(d["summary"][:300])}</div>

<!-- Stats bar -->
<div>
  <div class="stat-box">
    <div class="stat-label">阶段数</div>
    <div class="stat-val">{len(d["phases"])}</div>
  </div>
  <div class="stat-box">
    <div class="stat-label">招式/阵法数</div>
    <div class="stat-val">{len(d["skills"])}</div>
  </div>
  <div class="stat-box">
    <div class="stat-label">前置依赖</div>
    <div class="stat-val">{len(d["dependencies"])}</div>
  </div>
  <div class="stat-box">
    <div class="stat-label">开放问题</div>
    <div class="stat-val">{len(d["open_questions"])}</div>
  </div>
  <div class="stat-box">
    <div class="stat-label">测试下限</div>
    <div class="stat-val">{_esc(d["test_target"])}</div>
  </div>
</div>

<!-- Worldview anchors -->
<h2>世界观锚点</h2>
<div class="section">
  <ul>
    {wv_items if wv_items else '<li style="color:#555;">未提取到锚点</li>'}
  </ul>
</div>

<!-- Dependencies -->
<h2>前置依赖</h2>
<div class="section">
  <div class="dep-grid">
    <div>
      <table>
        <tr><th>依赖项</th></tr>
        {dep_rows if dep_rows else '<tr><td style="color:#555;">无</td></tr>'}
      </table>
    </div>
    <div>
      <table>
        <tr><th>反向被依赖</th></tr>
        {rev_rows if rev_rows else '<tr><td style="color:#555;">无</td></tr>'}
      </table>
    </div>
  </div>
</div>

<!-- Phase overview -->
<h2>阶段总览</h2>
<div class="section">
  <table>
    <tr><th>阶段</th><th>内容</th><th>验收标准</th></tr>
    {phase_rows if phase_rows else '<tr><td colspan="3" style="color:#555;">未提取到阶段表</td></tr>'}
  </table>
</div>

<!-- Skills/Moves -->
<h2>招式/阵法规格</h2>
<div class="section">
  <div class="skill-grid">
    {skill_cards if skill_cards else '<div style="color:#555;">未提取到招式</div>'}
  </div>
</div>

<!-- Open questions -->
<h2>开放问题 / 决策门</h2>
<div class="section">
  <ul>
    {oq_items if oq_items else '<li style="color:#555;">无开放问题</li>'}
  </ul>
</div>

<div class="footer">
  plan_overview_gen.py &middot; {d["filename"]} &middot; 骨架审阅页
</div>

</body>
</html>"""
    return html


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def parse_plan(filepath: Path) -> dict:
    text = filepath.read_text(encoding="utf-8")
    title = _first_heading(text)
    summary = _first_paragraph(text)
    worldview_anchors = _extract_worldview_anchors(text)
    dependencies = _extract_dependencies(text)
    reverse_deps = _extract_reverse_deps(text)

    # skills section: try multiple section patterns across plans
    # dugu/baomai/tuike use §2, zhenfa uses §1, poison-trait uses §3.5
    skills_section = ""
    for pat in [r"[§]2\s+(?:五招|三招|招)", r"[§]1\s+(?:五阵|三招|五招)", r"[§]3\.5", r"[§]2"]:
        skills_section = _extract_section(text, pat)
        if skills_section:
            break
    if not skills_section:
        # fallback: search for any section containing skill subheadings
        for pat in [r"(?:招|阵|规格)", r"毒丹"]:
            skills_section = _extract_section(text, pat)
            if skills_section and "###" in skills_section:
                break

    skills = _extract_skills(skills_section) if skills_section else []

    # phase table: search entire document (tables appear in various sections)
    phases = _extract_phase_table(text)

    # open questions: scan multiple possible section numbers
    open_questions = []
    for pat in [r"[§]5\s+开放", r"[§]6\s+(?:已知|开放|open)", r"[§]4\s+开放", r"开放问题", r"已知风险"]:
        oq_section = _extract_section(text, pat)
        if oq_section:
            open_questions = _extract_open_questions(oq_section)
            if open_questions:
                break
    # also try Q-RC style questions for tsy-raceout
    if not open_questions:
        for m in re.finditer(r"^-\s+\[\s*\]\s+\*\*(Q-RC\d+.+?)\*\*", text, re.MULTILINE):
            open_questions.append(m.group(1).strip())

    # test target from any section mentioning test matrix
    test_target = _extract_test_target(text)

    return {
        "filename": filepath.name,
        "title": title,
        "summary": summary,
        "worldview_anchors": worldview_anchors,
        "dependencies": dependencies,
        "reverse_deps": reverse_deps,
        "skills": skills,
        "phases": phases,
        "open_questions": open_questions,
        "test_target": test_target,
    }


def main():
    generated = []
    for fname in PLAN_FILES:
        src = SKELETON_DIR / fname
        if not src.exists():
            print(f"  [SKIP] {src} not found")
            continue

        data = parse_plan(src)
        html = generate_html(data)

        stem = fname.replace(".md", "")
        out_path = OUTPUT_DIR / f"plan-overview-{stem}.html"
        out_path.write_text(html, encoding="utf-8")
        generated.append(str(out_path))
        print(f"  [OK] {out_path}")

    print(f"\nGenerated {len(generated)} overview page(s).")
    return generated


if __name__ == "__main__":
    main()
