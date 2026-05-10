"""
qi_collision 流派对策模拟器 v2。

三套公式并排对比:
  - LEGACY:   旧代码 (resistance 二次减免 + purity 代理 ρ)
  - RUST_LIVE: 当前 Rust 公式 (resistance hard cap 0.95 + rejection_rate 替代 purity)
  - FIX_B:    备选公式：去掉 defender_lost 的 (1-r) 二次减免 + rejection_rate

输出交互式 HTML 热力图,可切公式版本 / 距离 / 指标。
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

# ─── qi_physics::constants ───────────────────────────────────────────────

QI_DECAY_PER_BLOCK = 0.03
QI_EXCRETION_BASE = 0.30
QI_ACOUSTIC_THRESHOLD = 0.40
QI_DRAIN_CLAMP = 0.50
QI_RHYTHM_NEUTRAL = 1.0

# ─── env.rs: MediumKind ──────────────────────────────────────────────────

COLOR_LOSS_BONUS = {
    "Sharp": 0.012, "Heavy": 0.004, "Mellow": 0.0, "Solid": -0.004,
    "Light": 0.018, "Intricate": 0.01, "Gentle": -0.002,
    "Insidious": 0.014, "Violent": 0.02, "Turbid": 0.024,
}
CARRIER_LOSS_BONUS = {
    "BareQi": 0.0, "PhysicalWeapon": 0.008,
    "SpiritWeapon": -0.006, "AncientRelic": -0.012,
}


@dataclass
class MediumKind:
    color: str = "Mellow"
    carrier: str = "BareQi"

    def loss_bonus_per_block(self) -> float:
        return COLOR_LOSS_BONUS.get(self.color, 0.0) + CARRIER_LOSS_BONUS.get(self.carrier, 0.0)


def qi_distance_atten(initial: float, distance: float, medium: MediumKind) -> float:
    if initial <= 0 or distance <= 0:
        return initial
    loss = max(0.0, min(0.95, QI_DECAY_PER_BLOCK + medium.loss_bonus_per_block()))
    return initial * (1.0 - loss) ** distance


# ─── collision variants ──────────────────────────────────────────────────

@dataclass
class CollisionOutcome:
    attenuated_qi: float
    effective_hit: float
    attacker_spent: float
    defender_lost: float
    defender_absorbed: float


def collision_current(
    injected_qi: float, purity: float, medium: MediumKind,
    resistance: float, drain_affinity: float, distance: float,
    rejection_rate: float,  # ignored in current
) -> CollisionOutcome:
    """旧公式 1:1 翻译，用来对照 Rust live 公式。"""
    injected = max(0.0, injected_qi)
    attenuated = qi_distance_atten(injected, distance, medium)
    p = max(0.0, min(1.0, purity))
    if p < QI_ACOUSTIC_THRESHOLD:
        return CollisionOutcome(attenuated, 0.0, injected, 0.0, 0.0)
    r = max(0.0, min(1.0, resistance))
    rejection = attenuated * QI_EXCRETION_BASE * (1.0 - p + r * 0.5)
    effective_hit = max(0.0, attenuated - rejection)
    defender_lost = effective_hit * (1.0 - r)
    da = max(0.0, min(1.0, drain_affinity))
    defender_absorbed = min(defender_lost * da, injected * QI_DRAIN_CLAMP)
    return CollisionOutcome(attenuated, effective_hit, injected, defender_lost, defender_absorbed)


def collision_fix_a(
    injected_qi: float, purity: float, medium: MediumKind,
    resistance: float, drain_affinity: float, distance: float,
    rejection_rate: float,
) -> CollisionOutcome:
    """方案 A: rejection_rate 替代 (1-purity) + resistance hard cap 0.95"""
    injected = max(0.0, injected_qi)
    attenuated = qi_distance_atten(injected, distance, medium)
    p = max(0.0, min(1.0, purity))
    if p < QI_ACOUSTIC_THRESHOLD:
        return CollisionOutcome(attenuated, 0.0, injected, 0.0, 0.0)
    r = max(0.0, min(1.0, resistance))
    rr = max(0.0, min(1.0, rejection_rate))
    rejection = attenuated * QI_EXCRETION_BASE * (rr + r * 0.5)
    effective_hit = max(0.0, attenuated - rejection)
    r_capped = min(r, 0.95)
    defender_lost = effective_hit * (1.0 - r_capped)
    da = max(0.0, min(1.0, drain_affinity))
    defender_absorbed = min(defender_lost * da, injected * QI_DRAIN_CLAMP)
    return CollisionOutcome(attenuated, effective_hit, injected, defender_lost, defender_absorbed)


def collision_fix_b(
    injected_qi: float, purity: float, medium: MediumKind,
    resistance: float, drain_affinity: float, distance: float,
    rejection_rate: float,
) -> CollisionOutcome:
    """方案 B: rejection_rate + 去掉 defender_lost 二次减免（rejection 阶段已过滤）"""
    injected = max(0.0, injected_qi)
    attenuated = qi_distance_atten(injected, distance, medium)
    p = max(0.0, min(1.0, purity))
    if p < QI_ACOUSTIC_THRESHOLD:
        return CollisionOutcome(attenuated, 0.0, injected, 0.0, 0.0)
    r = max(0.0, min(1.0, resistance))
    rr = max(0.0, min(1.0, rejection_rate))
    rejection = attenuated * QI_EXCRETION_BASE * (rr + r * 0.5)
    effective_hit = max(0.0, attenuated - rejection)
    defender_lost = effective_hit  # no second (1-r) multiplier
    da = max(0.0, min(1.0, drain_affinity))
    defender_absorbed = min(defender_lost * da, injected * QI_DRAIN_CLAMP)
    return CollisionOutcome(attenuated, effective_hit, injected, defender_lost, defender_absorbed)


FORMULA_VARIANTS = {
    "legacy": ("旧公式: purity 代理 ρ", collision_current),
    "rust_live": ("Rust core: neutral env/no backfire + cap 0.95 + rho", collision_fix_a),
    "fix_b": ("备选B: 去二次减免 + ρ", collision_fix_b),
}

# ─── 流派定义 ────────────────────────────────────────────────────────────

@dataclass
class AttackProfile:
    name: str
    cn_name: str
    color: str
    injected_qi: float
    purity: float
    rejection_rate: float  # worldview ρ
    medium: MediumKind
    note: str = ""


@dataclass
class DefenseProfile:
    name: str
    cn_name: str
    color: str
    resistance: float
    drain_affinity: float
    note: str = ""


# ── 攻击方 (加 rejection_rate) ──

BAOMAI = AttackProfile(
    name="Baomai", cn_name="体修·崩拳", color="Heavy",
    injected_qi=20.0, purity=0.85, rejection_rate=0.65,
    medium=MediumKind("Heavy", "BareQi"),
    note="qi=20, purity=0.85, ρ=0.65 (浑厚真元高排斥)",
)
ANQI_BONE = AttackProfile(
    name="Anqi(Bone)", cn_name="暗器·骨针", color="Sharp",
    injected_qi=8.0, purity=1.0, rejection_rate=0.45,
    medium=MediumKind("Sharp", "SpiritWeapon"),
    note="qi=8, ρ=0.45, SpiritWeapon 载体",
)
ANQI_BARE = AttackProfile(
    name="Anqi(Bare)", cn_name="暗器·裸针", color="Sharp",
    injected_qi=8.0, purity=1.0, rejection_rate=0.45,
    medium=MediumKind("Sharp", "BareQi"),
    note="qi=8, ρ=0.45, 无载体",
)
DUGU_NEEDLE = AttackProfile(
    name="Dugu(Needle)", cn_name="毒蛊·蚀针", color="Insidious",
    injected_qi=5.0, purity=0.9, rejection_rate=0.05,
    medium=MediumKind("Insidious", "BareQi"),
    note="qi=5, purity=0.9, ρ=0.05 (脏真元低排斥)",
)
DUGU_MELEE = AttackProfile(
    name="Dugu(Melee)", cn_name="毒蛊·近战注入", color="Insidious",
    injected_qi=5.0, purity=0.75, rejection_rate=0.05,
    medium=MediumKind("Insidious", "BareQi"),
    note="qi=5, purity=0.75, ρ=0.05",
)
WOLIU_SMALL = AttackProfile(
    name="Woliu(r=4)", cn_name="涡流·小涡", color="Intricate",
    injected_qi=6.0, purity=0.875, rejection_rate=0.30,
    medium=MediumKind("Intricate", "BareQi"),
    note="r=4, purity=0.875, ρ=0.30",
)
WOLIU_LARGE = AttackProfile(
    name="Woliu(r=16)", cn_name="涡流·大涡", color="Intricate",
    injected_qi=15.0, purity=0.5, rejection_rate=0.30,
    medium=MediumKind("Intricate", "BareQi"),
    note="r=16, purity=0.5, ρ=0.30",
)
ZHENFA_LOW = AttackProfile(
    name="Zhenfa(low)", cn_name="阵法·低投", color="Mellow",
    injected_qi=10.0, purity=0.4, rejection_rate=0.35,
    medium=MediumKind("Mellow", "PhysicalWeapon"),
    note="invest_ratio=0.4, ρ=0.35",
)
ZHENFA_HIGH = AttackProfile(
    name="Zhenfa(high)", cn_name="阵法·高投", color="Mellow",
    injected_qi=25.0, purity=0.8, rejection_rate=0.35,
    medium=MediumKind("Mellow", "SpiritWeapon"),
    note="invest_ratio=0.8, ρ=0.35, SpiritWeapon",
)

ALL_ATTACKS = [
    BAOMAI, ANQI_BONE, ANQI_BARE, DUGU_NEEDLE, DUGU_MELEE,
    WOLIU_SMALL, WOLIU_LARGE, ZHENFA_LOW, ZHENFA_HIGH,
]

# ── 防御方 ──

NO_DEFENSE = DefenseProfile(
    name="None", cn_name="无防御", color="Mellow",
    resistance=0.0, drain_affinity=0.0, note="裸体 baseline",
)
JIEMAI_CONDENSE = DefenseProfile(
    name="Jiemai(Condense)", cn_name="截脉·凝脉", color="Solid",
    resistance=0.6, drain_affinity=0.2, note="realm_f=0.6",
)
JIEMAI_SPIRIT = DefenseProfile(
    name="Jiemai(Spirit)", cn_name="截脉·通灵", color="Solid",
    resistance=1.0, drain_affinity=0.2, note="realm_f=1.0 → 现行无敌",
)
JIEMAI_SPIRIT_HEAVY = DefenseProfile(
    name="Jiemai(Spirit+Heavy)", cn_name="截脉·通灵重甲", color="Solid",
    resistance=1.0, drain_affinity=0.2, note="realm_f=1.0+armor → clamped 1.0",
)
TUIKE_SILK = DefenseProfile(
    name="Tuike(Silk)", cn_name="替尸·蛛丝", color="Gentle",
    resistance=10.0 / 30.0, drain_affinity=0.15, note="contam_cap=10 → res=0.333",
)
TUIKE_WOOD = DefenseProfile(
    name="Tuike(Wood)", cn_name="替尸·朽木甲", color="Solid",
    resistance=1.0, drain_affinity=0.05, note="contam_cap=30 → res=1.0 现行无敌",
)
ZHENFA_DEF_LOW = DefenseProfile(
    name="ZhenfaDef(r=4)", cn_name="阵法防御·小阵", color="Mellow",
    resistance=0.25, drain_affinity=0.1, note="ward_r=4 → res=0.25",
)
ZHENFA_DEF_HIGH = DefenseProfile(
    name="ZhenfaDef(r=12)", cn_name="阵法防御·大阵", color="Mellow",
    resistance=0.75, drain_affinity=0.2, note="ward_r=12 → res=0.75",
)

ALL_DEFENSES = [
    NO_DEFENSE, JIEMAI_CONDENSE, JIEMAI_SPIRIT, JIEMAI_SPIRIT_HEAVY,
    TUIKE_SILK, TUIKE_WOOD, ZHENFA_DEF_LOW, ZHENFA_DEF_HIGH,
]

# ─── 模拟 ───────────────────────────────────────────────────────────────

DISTANCES = [0, 1, 3, 5, 10, 20, 40]


def run_all():
    """返回 { formula_key: { dist_str: [rows] } }"""
    result = {}
    for fkey, (_, fn) in FORMULA_VARIANTS.items():
        by_dist = {}
        for d in DISTANCES:
            rows = []
            for atk in ALL_ATTACKS:
                for dfn in ALL_DEFENSES:
                    out = fn(
                        injected_qi=atk.injected_qi,
                        purity=atk.purity,
                        medium=atk.medium,
                        resistance=dfn.resistance,
                        drain_affinity=dfn.drain_affinity,
                        distance=d,
                        rejection_rate=atk.rejection_rate,
                    )
                    rows.append({
                        "atk": atk.cn_name,
                        "def": dfn.cn_name,
                        "atk_qi": atk.injected_qi,
                        "atk_purity": atk.purity,
                        "atk_rr": atk.rejection_rate,
                        "def_res": dfn.resistance,
                        "def_drain": dfn.drain_affinity,
                        "distance": d,
                        "attenuated": round(out.attenuated_qi, 3),
                        "effective_hit": round(out.effective_hit, 3),
                        "defender_lost": round(out.defender_lost, 3),
                        "defender_absorbed": round(out.defender_absorbed, 3),
                        "efficiency": round(
                            out.defender_lost / out.attacker_spent * 100
                            if out.attacker_spent > 0 else 0, 1
                        ),
                    })
            by_dist[str(d)] = rows
        result[fkey] = by_dist
    return result


# ─── HTML ────────────────────────────────────────────────────────────────

def generate_html(all_data: dict) -> str:
    atk_names = [a.cn_name for a in ALL_ATTACKS]
    def_names = [d.cn_name for d in ALL_DEFENSES]
    formula_labels = {k: v[0] for k, v in FORMULA_VARIANTS.items()}

    # attack/defense param tables
    atk_rows_html = ""
    for a in ALL_ATTACKS:
        atk_rows_html += (
            f'<tr><td class="note-label">{a.cn_name}</td>'
            f'<td>{a.color}</td><td>{a.injected_qi}</td>'
            f'<td>{a.purity}</td><td>{a.rejection_rate}</td>'
            f'<td>{a.medium.carrier}</td><td>{a.note}</td></tr>\n'
        )
    def_rows_html = ""
    for d in ALL_DEFENSES:
        def_rows_html += (
            f'<tr><td class="note-label">{d.cn_name}</td>'
            f'<td>{d.resistance:.3f}</td><td>{d.drain_affinity}</td>'
            f'<td>{d.note}</td></tr>\n'
        )

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>Bong 流派对策模拟 v2 — Rust live 公式对比</title>
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: "SF Mono", "Cascadia Code", "Consolas", monospace; background: #0a0a0a; color: #e0e0e0; padding: 24px; }}
h1 {{ font-size: 20px; margin-bottom: 4px; color: #c9a96e; }}
h2 {{ font-size: 15px; color: #c9a96e; margin: 20px 0 8px; }}
.subtitle {{ color: #888; font-size: 13px; margin-bottom: 16px; }}

.controls {{ display: flex; gap: 12px; align-items: center; margin-bottom: 14px; flex-wrap: wrap; }}
.controls label {{ font-size: 13px; color: #aaa; }}
.controls select {{ background: #1a1a1a; border: 1px solid #333; color: #e0e0e0; padding: 4px 8px; border-radius: 4px; font-family: inherit; }}
.btn {{ background: #1a1a1a; border: 1px solid #444; color: #ccc; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 12px; font-family: inherit; }}
.btn.active {{ background: #c9a96e; color: #000; border-color: #c9a96e; }}
.btn.formula-btn.active {{ background: #4a7c59; color: #fff; border-color: #4a7c59; }}

.grid-container {{ overflow-x: auto; }}
table.matrix {{ border-collapse: collapse; font-size: 12px; }}
table.matrix th, table.matrix td {{ padding: 6px 10px; text-align: center; border: 1px solid #222; white-space: nowrap; }}
table.matrix th {{ background: #1a1a1a; color: #c9a96e; font-weight: 600; position: sticky; top: 0; z-index: 2; }}
table.matrix td.rh {{ text-align: right; position: sticky; left: 0; background: #111; font-weight: 500; color: #ddd; min-width: 130px; z-index: 1; }}
table.matrix td.val {{ min-width: 72px; font-variant-numeric: tabular-nums; }}
table.matrix .corner {{ position: sticky; left: 0; top: 0; z-index: 4; background: #1a1a1a; }}

.formula-box {{ background: #111; border: 1px solid #333; border-radius: 6px; padding: 14px 18px; margin: 12px 0; font-size: 13px; line-height: 1.8; color: #ccc; }}
.formula-box code {{ color: #e0c080; }}
.formula-box .fix {{ color: #6dba82; font-weight: 600; }}
.formula-box .old {{ color: #b05050; text-decoration: line-through; }}

.notes table {{ border-collapse: collapse; font-size: 12px; margin-bottom: 16px; }}
.notes th {{ background: #1a1a1a; color: #c9a96e; padding: 4px 10px; border: 1px solid #222; }}
.notes td {{ text-align: left; padding: 4px 10px; border: 1px solid #222; }}
.note-label {{ color: #c9a96e; font-weight: 600; }}

.insight {{ background: #1a1500; border-left: 3px solid #c9a96e; padding: 10px 14px; margin: 10px 0; font-size: 13px; line-height: 1.6; }}
.insight strong {{ color: #c9a96e; }}
</style>
</head>
<body>
<h1>Bong · qi_collision 流派对策 — Rust live 公式涌现对比</h1>
<p class="subtitle">旧公式 vs Rust core neutral env/no backfire (resistance cap 0.95 + rejection_rate rho) vs 备选B (去 defender_lost 二次减免 + rho)</p>

<div class="formula-box">
<strong>rejection 公式变化</strong><br>
<span class="old"><code>rejection = attenuated × 0.30 × (1 - purity + resistance × 0.5)</code></span> 现行: purity 代理 ρ<br>
<span class="fix"><code>rejection = attenuated × 0.30 × (rejection_rate + resistance × 0.5)</code></span> 修正: rejection_rate (ρ) 独立参数<br><br>
<strong>defender_lost 变化</strong><br>
<span class="old"><code>defender_lost = effective_hit × (1 - resistance)</code></span> 旧公式: resistance 二次减免<br>
<span class="fix"><code>Rust core neutral env/no backfire: defender_lost = effective_hit × (1 - min(resistance, 0.95))</code></span> hard cap 95%<br>
<span class="fix"><code>方案B: defender_lost = effective_hit</code></span> 去掉二次减免,rejection 阶段已过滤
</div>

<div class="controls">
  <label>公式:</label>
  <button class="btn formula-btn" data-formula="legacy">旧公式</button>
  <button class="btn formula-btn active" data-formula="rust_live">Rust core: neutral env/no backfire</button>
  <button class="btn formula-btn" data-formula="fix_b">方案B: 去减免+ρ</button>

  <label style="margin-left:16px">距离:</label>
  <select id="distSel">
    {"".join(f'<option value="{d}"{"selected" if d == 3 else ""}>{d} blocks</option>' for d in DISTANCES)}
  </select>

  <span style="margin-left:16px">指标:</span>
  <button class="btn metric-btn active" data-metric="defender_lost">defender_lost</button>
  <button class="btn metric-btn" data-metric="effective_hit">effective_hit</button>
  <button class="btn metric-btn" data-metric="efficiency">efficiency %</button>
  <button class="btn metric-btn" data-metric="defender_absorbed">absorbed</button>
</div>

<div class="grid-container"><table class="matrix" id="matrix"></table></div>

<div class="notes">
<h2>攻击方参数</h2>
<table><tr><th>流派</th><th>color</th><th>qi</th><th>purity</th><th>ρ (rejection_rate)</th><th>carrier</th><th>备注</th></tr>
{atk_rows_html}</table>

<h2>防御方参数</h2>
<table><tr><th>流派</th><th>resistance</th><th>drain_affinity</th><th>备注</th></tr>
{def_rows_html}</table>
</div>

<script>
const ALL_DATA = {json.dumps(all_data)};
const ATK = {json.dumps(atk_names)};
const DEF = {json.dumps(def_names)};
const FORMULA_LABELS = {json.dumps(formula_labels)};

let curFormula = 'rust_live';
let curMetric = 'defender_lost';

function colorScale(val, maxVal) {{
  if (maxVal <= 0) return 'rgba(40,40,40,0.5)';
  const t = Math.min(1, val / maxVal);
  if (t < 0.005) return 'rgba(30,10,10,0.6)';
  const r = Math.round(60 + 140 * t);
  const g = Math.round(20 + 130 * Math.pow(t, 0.6));
  const b = Math.round(10 + 30 * Math.pow(t, 2));
  return `rgba(${{r}},${{g}},${{b}},${{0.3 + 0.7*t}})`;
}}

function render() {{
  const dist = document.getElementById('distSel').value;
  const rows = ALL_DATA[curFormula]?.[dist];
  if (!rows) return;

  const lk = {{}};
  let mx = 0;
  for (const r of rows) {{
    const v = r[curMetric];
    lk[r.atk + '|' + r.def] = v;
    if (v > mx) mx = v;
  }}

  const t = document.getElementById('matrix');
  let h = '<tr><th class="corner">攻 ＼ 防</th>';
  for (const d of DEF) h += `<th>${{d}}</th>`;
  h += '</tr>';
  for (const a of ATK) {{
    h += `<tr><td class="rh">${{a}}</td>`;
    for (const d of DEF) {{
      const v = lk[a + '|' + d] ?? 0;
      const bg = colorScale(v, mx);
      const s = curMetric === 'efficiency' ? v.toFixed(1) + '%' : v.toFixed(2);
      h += `<td class="val" style="background:${{bg}}">${{s}}</td>`;
    }}
    h += '</tr>';
  }}
  t.innerHTML = h;
}}

document.getElementById('distSel').addEventListener('change', render);
document.querySelectorAll('.formula-btn').forEach(b => {{
  b.addEventListener('click', () => {{
    document.querySelectorAll('.formula-btn').forEach(x => x.classList.remove('active'));
    b.classList.add('active');
    curFormula = b.dataset.formula;
    render();
  }});
}});
document.querySelectorAll('.metric-btn').forEach(b => {{
  b.addEventListener('click', () => {{
    document.querySelectorAll('.metric-btn').forEach(x => x.classList.remove('active'));
    b.classList.add('active');
    curMetric = b.dataset.metric;
    render();
  }});
}});

render();
</script>
</body>
</html>"""
    return html


# ─── main ────────────────────────────────────────────────────────────────

def main():
    all_data = run_all()

    out = Path(__file__).parent / "style_collision_sim.html"
    out.write_text(generate_html(all_data), encoding="utf-8")
    print(f"wrote {out}  ({out.stat().st_size / 1024:.1f} KB)")

    # summary: distance=3
    for fkey, (label, _) in FORMULA_VARIANTS.items():
        print(f"\n=== {label} | distance=3 | defender_lost ===")
        rows = all_data[fkey]["3"]
        an = [a.cn_name for a in ALL_ATTACKS]
        dn = [d.cn_name for d in ALL_DEFENSES]
        hdr = f"{'':>16}" + "".join(f"{d:>12}" for d in dn)
        print(hdr)
        for a in an:
            vals = "".join(
                f"{next(r for r in rows if r['atk']==a and r['def']==d)['defender_lost']:>12.2f}"
                for d in dn
            )
            print(f"{a:>16}{vals}")


if __name__ == "__main__":
    main()
