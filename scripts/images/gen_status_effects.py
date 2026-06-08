#!/usr/bin/env python3
"""批量生成战斗状态效果 HUD 图标（status effect icons）。

每个 effect → 一张高对比扁平 emblem 图标，1024 透明底生成，缩到 256×256
落入 client 资产。文件名 **必须** 等于 server `status_effect_id()` 的输出
（见 server/src/network/status_snapshot_emit.rs）——大多数变体走
`format!("{:?}").to_ascii_lowercase()`（PascalCase → 全小写无下划线，如
`DamageReduction` → `damagereduction`）；少数显式 arm 带下划线/冒号
（`qi_regen_paused`、`body_part_resist:<part>` 等，带参数的取冒号前的基名）。

用法：

    # 生成所有缺失的图标（已存在的跳过）
    python3 scripts/images/gen_status_effects.py

    # 只生成某几个（按 id）
    python3 scripts/images/gen_status_effects.py --only damageamp humility

    # 强制重生成（覆盖）
    python3 scripts/images/gen_status_effects.py --force --only frailty

    # 把 local_images/effects/*.png 缩到 256 装进 client 资产
    python3 scripts/images/gen_status_effects.py --install

并发由 --jobs 控制（默认 4），避免打爆图片后端速率。
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
GEN = SCRIPT_DIR / "gen.py"
OUT_DIR = REPO_ROOT / "local_images" / "effects"
ASSET_DIR = (
    REPO_ROOT
    / "client/src/main/resources/assets/bong-client/textures/hud/effects"
)
ASSET_SIZE = 256

# 共享 emblem 画风：扁平、高对比、单母题、小尺寸仍可读。状态身份靠母题 + 颜色，
# 类别色由 HUD 槽边框另行承载，这里只管图标本体。
STYLE = (
    "bold flat game status-effect emblem icon, single centered symbol, "
    "thick clean silhouette, high contrast, limited palette, minimal internal "
    "detail, must stay readable when scaled down to 16 pixels, dark xianxia "
    "aesthetic with a subtle inner glow, no text, no letters, no frame, no "
    "border ring, fully transparent background alpha 0, centered, no drop shadow"
)

# (id 文件名, 类别, 母题描述)。母题彼此必须视觉可区分（同类不同形/不同色调）。
EFFECTS: list[tuple[str, str, str]] = [
    # ---- DoT (红) ----
    ("bleeding", "dot",
     "three deep crimson blood droplets falling together, the largest droplet "
     "in front, glossy dark blood-red with one sharp white highlight, a sense "
     "of bleeding wound"),
    # ---- Control (紫) ----
    ("stunned", "control",
     "a dizzying violet vertigo spiral swirl with two small four-point stars "
     "orbiting around it, a stunned disoriented feeling, glowing amethyst "
     "purple and pale lavender"),
    ("vortexcasting", "control",
     "a dark inward-sucking funnel vortex pulling spirit-energy into its "
     "center, concentric spiral walls collapsing toward a black core, deep "
     "indigo and violet, a qi-sealing whirlpool"),
    ("parryrecovery", "control",
     "a curved sabre being drawn back into a tight guard stance, two crossed "
     "motion arcs showing the recovery sweep, cold steel-violet, a meridian-cut "
     "parry recovery"),
    ("staggered", "control",
     "a jagged impact starburst crack with a stiff angular figure jolting "
     "backward from the hit, violet-white shockwave lines, a recoil stagger"),
    ("disoriented", "control",
     "a tangled looping knot of scribbled spirals wrapping a small confused "
     "dot at the center, messy lavender threads, a confused mind, clearly "
     "different from a clean spiral"),
    ("voidcoreactive", "control",
     "an imploding black sphere with four short inward-pointing arrows around "
     "it, a collapsing void heart, pure black core ringed with thin violet "
     "light, negative-pressure collapse"),
    # ---- Buff (绿) ----
    ("damagereduction", "buff",
     "a jade-green hexagonal protective barrier shield with a faint qi shimmer "
     "across its surface, a defensive ward, solid emerald jade green with soft "
     "inner glow"),
    ("breakthroughboost", "buff",
     "an upward-bursting arrow smashing through a cracked stone barrier ring, "
     "shards flying outward, radiant green and gold, a cultivation breakthrough "
     "surge"),
    ("antispiritpressurepill", "buff",
     "a downward heavy pressure wave being pushed back by a rising dome, two "
     "opposing vertical arrows pressure-versus-resistance, teal-green, "
     "withstanding spirit pressure"),
    ("qiregenboost", "buff",
     "a swirling green qi spiral feeding upward sparks into a bright core, "
     "small rising motes, fresh vivid green, regenerating energy"),
    ("insightflash", "buff",
     "a single calm eye opening with sharp radiating enlightenment rays, a "
     "spark of sudden insight at the pupil, pale gold-green glow"),
    ("woundheal", "buff",
     "a soft green plus/cross over a closing gash mended with a single stitch "
     "thread, a stabilizing wound, gentle healing green"),
    ("body_part_resist", "buff",
     "a simple humanoid torso silhouette with one body segment overlaid by a "
     "glowing hardened jade plate, body-part hardening, calm jade green"),
    ("speedboost", "buff",
     "three forward-leaning speed chevrons with thin motion streaks trailing "
     "behind, swift movement, bright vivid green"),
    ("staminarecovboost", "buff",
     "a coiled spring releasing upward with a rising fill arrow beside it, "
     "stamina recovery, lime green energetic glow"),
    ("mirror_concealment", "buff",
     "a translucent ghostly figure dissolving into a faint mirror shimmer, "
     "half-faded outline, stealth concealment, ethereal green-cyan"),
    ("swordparrying", "buff",
     "an upright vertical sword held in a ready guard with a bright deflection "
     "spark flashing at the cross-guard, steel green-white, a parry stance, "
     "clearly a standing blade not a drawn-back one"),
    ("spirit_treasure_perception", "buff",
     "concentric radar sensing rings expanding outward from a small faceted "
     "gem at the center, spirit-treasure detection, green-cyan glow"),
    ("cultivationacceleration", "buff",
     "a seated meditating figure silhouette with an upward spiral of "
     "accelerating qi rising from it and two small fast-forward arcs, serene "
     "green, faster cultivation"),
    ("extraordinarymeridianacceleration", "buff",
     "a branching network of glowing meridian channels with a double "
     "fast-forward chevron over it, extraordinary-meridian acceleration, deep "
     "green-gold, distinct from a meditating figure"),
    # ---- Debuff (橙) ----
    ("slowed", "debuff",
     "a heavy iron anchor weighing downward with two slow downward chevrons "
     "beside it, sluggish movement, dull muted orange"),
    ("damageamp", "debuff",
     "a round target crosshair with a jagged spiking arrow stabbing upward "
     "through its center, amplified incoming damage, hot orange-red"),
    ("humility", "debuff",
     "a flat downward-pressing palm pushing a small bowed figure lower, "
     "suppression and humbling, muted amber"),
    ("insighthallucination", "debuff",
     "a fractured swirling eye splitting into doubled ghost after-images with "
     "wavy distortion lines, hallucination, sickly orange tinged with a faint "
     "violet edge"),
    ("frailty", "debuff",
     "a guttering candle flame bending and almost blown out, a thin wisp of "
     "smoke rising, candle-in-the-wind frailty, dim weak amber"),
    ("qicappermminus", "debuff",
     "a cracked dim spirit-energy orb fracturing apart with a downward arrow "
     "beside it and grey wisps leaking out, permanent loss of maximum power, "
     "muted amber-orange and ash grey, dull and broken"),
    ("contaminationboost", "debuff",
     "a tilted alchemy vial leaking a bubbling corrupted droplet with rising "
     "toxic fumes, worsening pill toxin, sickly orange shot with bile-green"),
    ("body_part_weaken", "debuff",
     "a simple humanoid torso silhouette with one body segment cracking and "
     "crumbling into fragile fissures, body-part weakening, brittle orange, "
     "distinct from a hardened plate"),
    ("staminacrash", "debuff",
     "an empty vertical gauge collapsing to the bottom with a drooping wilting "
     "arrow, total stamina exhaustion, washed-out grey-orange"),
    ("qidrainforstamina", "debuff",
     "two opposing curved arrows converting a cool spirit swirl downward into "
     "a warm body glow, qi traded for stamina, an exchange motif of teal into "
     "orange"),
    ("legstrain", "debuff",
     "a single bent leg outline with a sharp red stress-fracture crack at the "
     "shin, leg strain injury, orange-red ache mark"),
    ("qi_regen_paused", "debuff",
     "a bold pause symbol of two thick vertical bars over a dimmed faded qi "
     "swirl, qi regeneration halted, muted greyish orange"),
    ("mirror_exposed", "debuff",
     "a shattering mirror cracking apart to reveal a sharply highlighted "
     "figure behind it, concealment broken and exposed, alarm orange-red"),
    ("resonancelocked", "debuff",
     "a closed padlock sitting over concentric resonance sound-wave rings, "
     "resonance locked out, locked-down orange"),
    ("qiregenslowed", "debuff",
     "a single downward slow chevron over a sluggish thinning qi swirl, qi "
     "regen slowed, dull orange, distinct from a pause symbol"),
    ("damagevulnerability", "debuff",
     "a broken cracked guard shield split down the middle with a small target "
     "mark showing through the gap, vulnerable to damage, orange-red, distinct "
     "from an intact shield"),
    # ---- Unknown (灰) ----
    ("alchemy_buff", "unknown",
     "a single rounded alchemy pill capsule with a faint swirl of unknown "
     "residual energy around it, a generic pill side-effect, neutral grey-gold"),
]


def build_cmd(eid: str, motif: str) -> list[str]:
    """构造 gen.py 命令（纯函数，便于测试）。"""
    prompt = f"{STYLE} — motif: {motif}"
    return [
        sys.executable, str(GEN), prompt,
        "--name", eid, "--style", "none", "--transparent",
        "--size", "1024x1024", "--out", str(OUT_DIR), "--save-prompt",
    ]


def gen_one(eid: str, motif: str, *, force: bool) -> tuple[str, bool, str]:
    out = OUT_DIR / f"{eid}.png"
    if out.exists() and not force:
        return (eid, True, "skip (exists)")
    try:
        proc = subprocess.run(
            build_cmd(eid, motif), capture_output=True, text=True, timeout=360
        )
    except subprocess.TimeoutExpired:
        # gen.py 的 urllib 自带 ~300s 读超时；这层兜底防整批卡死。
        return (eid, False, "TIMEOUT after 360s")
    ok = out.exists() and proc.returncode == 0
    tail = (proc.stderr or proc.stdout or "").strip().splitlines()
    return (eid, ok, tail[-1] if tail else ("ok" if ok else "FAILED"))


def install() -> tuple[int, list[str]]:
    """缩到 256 装进 client 资产；返回 (装入数, 缺失 id 列表)。"""
    from PIL import Image

    ASSET_DIR.mkdir(parents=True, exist_ok=True)
    ids = {e[0] for e in EFFECTS}
    n = 0
    missing: list[str] = []
    for eid in sorted(ids):
        src = OUT_DIR / f"{eid}.png"
        if not src.exists():
            missing.append(eid)
            continue
        im = Image.open(src).convert("RGBA").resize(
            (ASSET_SIZE, ASSET_SIZE), Image.LANCZOS
        )
        im.save(ASSET_DIR / f"{eid}.png")
        n += 1
    try:
        shown = ASSET_DIR.relative_to(REPO_ROOT)
    except ValueError:
        shown = ASSET_DIR
    print(f"installed {n} icons → {shown}")
    if missing:
        print(f"MISSING ({len(missing)}): {', '.join(missing)}", file=sys.stderr)
    return (n, missing)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--only", nargs="*", help="只处理这些 id")
    ap.add_argument("--force", action="store_true", help="覆盖已存在的")
    ap.add_argument("--jobs", type=int, default=4, help="并发数（默认 4）")
    ap.add_argument("--install", action="store_true",
                    help="缩到 256 装进 client 资产（不生成）")
    args = ap.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    if args.install:
        install()
        return

    todo = EFFECTS
    if args.only:
        want = set(args.only)
        todo = [e for e in EFFECTS if e[0] in want]
        unknown = want - {e[0] for e in EFFECTS}
        if unknown:
            print(f"未知 id: {', '.join(sorted(unknown))}", file=sys.stderr)

    print(f"生成 {len(todo)} 个图标（jobs={args.jobs}, force={args.force}）→ {OUT_DIR}")
    results: list[tuple[str, bool, str]] = []
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futs = [pool.submit(gen_one, eid, motif, force=args.force)
                for eid, _cat, motif in todo]
        for f in futs:
            eid, ok, msg = f.result()
            mark = "✓" if ok else "✗"
            print(f"  {mark} {eid}: {msg}")
            results.append((eid, ok, msg))

    failed = [eid for eid, ok, _ in results if not ok]
    if failed:
        print(f"\n失败 {len(failed)}: {', '.join(failed)}", file=sys.stderr)
        sys.exit(1)
    print(f"\n完成 {len(results)} 个。下一步：--install 装进资产")


if __name__ == "__main__":
    main()
