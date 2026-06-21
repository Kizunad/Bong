#!/usr/bin/env python3
"""批量生成功法技能图标。

每个功法 → gen.py(item 画风) → 缩 128×128 → 落到
client/.../assets/bong-client/textures/gui/items/skill_scroll_<safe_id>.png
（HUD 的 skill_scroll 候选路径 + 查看界面 ItemIconRegistry 同时命中）。

功法清单权威来源 server/src/cultivation/known_techniques.rs；此处内嵌
玩家可绑定功法的出图 prompt（跳过 npc.* 三个 NPC 专用技能）。

用法：
    python3 scripts/images/gen_technique_icons.py            # 全量（跳过已存在）
    python3 scripts/images/gen_technique_icons.py --only woliu.heart sword.cleave
    python3 scripts/images/gen_technique_icons.py --force    # 重出已存在的
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

from PIL import Image

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
OUT_DIR = (
    REPO_ROOT
    / "client/src/main/resources/assets/bong-client/textures/gui/items"
)
RAW_DIR = REPO_ROOT / "local_images"
ICON_SIZE = 128

# (technique_id, 中文名, 视觉 prompt)。style 前缀由 gen.py --style item 自动拼，
# 这里只写主体 + 关键母题 + 配色，统一 solid black background。
TECHNIQUES: list[tuple[str, str, str]] = [
    # —— 基础剑技 ——
    ("sword.cleave", "劈",
     "a heavy jian sword raised overhead mid-downward-cleave, a single broad arc of pale steel light "
     "tracing the chop, solid black background"),
    ("sword.thrust", "刺",
     "a straight jian sword thrusting forward point-first, a thin concentrated white piercing line "
     "along the blade tip, solid black background"),
    ("sword.parry", "格",
     "a jian sword held diagonally in a guard parry stance, a bright spark of deflected impact "
     "bursting where an unseen blow lands, solid black background"),
    ("sword.infuse", "注剑",
     "a jian sword blade wreathed in flowing spiritual qi, faint violet corrupted energy seeping "
     "into the steel along the fuller, solid black background"),
    ("movement.dash", "闪避",
     "a blurred dashing motion streak, a pale afterimage silhouette of a stepping figure with "
     "speed lines, solid black background"),
    ("shield_block", "盾挡",
     "a round battered iron shield raised front-on absorbing an impact, faint ripple of force "
     "spreading across its face, solid black background"),
    # —— 爆脉短打 ——
    ("burst_meridian.beng_quan", "崩拳",
     "a clenched fist thrust forward with a violent burst of force from the knuckles, cracked "
     "reddish meridian-energy streaks tearing up the forearm, solid black background"),
    ("burst_meridian.tie_shan_kao", "贴山靠",
     "a hunched shoulder body-slam lunge, a heavy blunt shockwave bursting from the upper arm and "
     "torso, dust and force lines, solid black background"),
    ("burst_meridian.xue_beng_bu", "血崩步",
     "an explosive forward lunging step, a leg trailing cracked crimson energy and motion blur as "
     "it bursts into a charge, solid black background"),
    ("burst_meridian.ni_mai_hu_ti", "逆脉护体",
     "a human torso silhouette guarded by reversed inward-spiraling qi currents forming a faint "
     "protective shell over the vital core, solid black background"),
    # —— 爆裂蓄放 ——
    ("baomai.full_power_charge", "全力一击·蓄",
     "a fist drawn back gathering a tightening sphere of dense compressed qi, concentric energy "
     "rings converging inward, building tension, solid black background"),
    ("baomai.full_power_release", "全力一击·放",
     "a fully released devastating punch, an explosive burst of white shockwave energy detonating "
     "outward from the knuckle, solid black background"),
    # —— 涡流（缺专属的几个）——
    ("woliu.vacuum_palm", "吸涡掌",
     "an open palm facing forward conjuring a swirling spiral vortex of pale violet "
     "negative-pressure air being sucked inward toward the palm center, solid black background"),
    ("woliu.vortex_shield", "涡流护体",
     "a swirling translucent dome of pale violet vacuum air wrapped protectively around a figure, "
     "deflecting incoming wisps, solid black background"),
    ("woliu.vacuum_lock", "真空锁",
     "a collapsing cage of violet vacuum lines clamping shut around a trapped point, an imploding "
     "low-pressure prison, solid black background"),
    ("woliu.vortex_resonance", "涡流共振",
     "a large spiral vortex centered on a figure pulling multiple targets inward, concentric "
     "resonating rings of pale violet low-pressure wind, solid black background"),
    ("woliu.turbulence_burst", "紊流爆发",
     "a shattering vacuum field exploding outward into violent chaotic turbulence, jagged "
     "pale-violet and white blast shards, solid black background"),
    # —— 毒蛊飞针 ——
    ("dugu.shoot_needle", "凝针",
     "a single hair-thin needle of condensed translucent spiritual qi with a sharp glinting tip, "
     "pale blue energy condensing along its length, solid black background"),
    ("dugu.infuse_poison", "灌毒蛊",
     "a thin flying needle coated in dissonant corrupted qi, sickly green-black poison residue "
     "crawling along its tip, solid black background"),
    # —— 蜕壳 ——
    ("tuike.don", "着壳",
     "a translucent false-skin shell molded over a figure like a shedding husk, faint outline of a "
     "second hollow layer of skin, solid black background"),
    ("tuike.shed", "蜕一层",
     "a hollow translucent cicada-like husk shell cracking and splitting open, an empty discarded "
     "outer shell layer crumbling away into faint dark wisps, solid black background"),
    ("tuike.transfer_taint", "转移污染",
     "violet-black corruption draining out of a glowing core into a surrounding translucent hollow "
     "husk shell, dark taint migrating into a discardable cicada-like outer shell, "
     "solid black background"),
    # —— 暗器封骨 ——
    ("anqi.charge_carrier", "封骨",
     "a pale mutated beast bone being sealed with stored spiritual qi, faint glowing runic charge "
     "soaking into the bone, solid black background"),
    ("anqi.single_snipe", "单射狙击",
     "a single sharpened bone dart streaking on a long precise trajectory line, a sniper shot "
     "loaded with sealed qi, solid black background"),
    ("anqi.multi_shot", "多发齐射",
     "a fan of five sharpened wooden dart projectiles fired in a spreading volley, parallel "
     "trajectory lines, solid black background"),
    ("anqi.soul_inject", "凝魂注射",
     "a dense colored crystalline dart loaded with condensed soul-essence, a glowing colored core "
     "pulsing inside, solid black background"),
    ("anqi.armor_pierce", "破甲注射",
     "a heavy bone dart in a reinforced casing overcharged to pierce armor, cracks of strained "
     "energy along its shaft, red-hot tip, solid black background"),
    ("anqi.echo_fractal", "诱饵分形",
     "a single bone dart fracturing into multiple ghostly fractal echo copies fanning out, "
     "translucent decoy duplicates, solid black background"),
    # —— 剑道 ——
    ("sword_path.condense_edge", "剑意·凝锋",
     "a jian blade edge gathering condensed solid sword-intent, a razor sheen of focused white edge "
     "energy forming along the cutting edge, solid black background"),
    ("sword_path.qi_slash", "剑气·斩",
     "a crescent blade of projected sword qi flying through the air, a thin white energy slash "
     "arc launched forward, solid black background"),
    ("sword_path.resonance", "共鸣·剑鸣",
     "a jian sword humming with resonating sound rings radiating outward, concentric vibration "
     "waves of pale ringing light, solid black background"),
    ("sword_path.manifest", "归一·剑意化形",
     "a manifested ghostly sword-spirit blade hovering on its own, condensed translucent sword "
     "intent given autonomous form, solid black background"),
    ("sword_path.heaven_gate", "天门·一剑开天",
     "a single colossal sword slash splitting the sky, a towering vertical rift of white light "
     "cleaving darkness, forbidden ultimate technique, solid black background"),
]


def safe_id(tid: str) -> str:
    return tid.replace(".", "_").replace(":", "_").replace("/", "_")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", help="只生成这些 technique id")
    ap.add_argument("--force", action="store_true", help="重出已存在的图标")
    ap.add_argument("--backend", default="cliproxy")
    args = ap.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    items = TECHNIQUES
    if args.only:
        want = set(args.only)
        items = [t for t in items if t[0] in want]

    done, skipped, failed = [], [], []
    for tid, name, prompt in items:
        target = OUT_DIR / f"skill_scroll_{safe_id(tid)}.png"
        if target.exists() and not args.force:
            skipped.append(tid)
            print(f"[skip] {tid} 已存在 → {target.name}")
            continue
        print(f"[gen ] {tid} ({name}) …", flush=True)
        raw_name = f"sk_{safe_id(tid)}"
        cmd = [
            sys.executable, str(SCRIPT_DIR / "gen.py"), prompt,
            "--name", raw_name, "--style", "item", "--backend", args.backend,
        ]
        r = subprocess.run(cmd, capture_output=True, text=True)
        raw = RAW_DIR / f"{raw_name}.png"
        if r.returncode != 0 or not raw.exists():
            failed.append(tid)
            print(f"[FAIL] {tid}: {r.stderr.strip().splitlines()[-1] if r.stderr.strip() else 'no output'}")
            continue
        img = Image.open(raw).convert("RGBA").resize((ICON_SIZE, ICON_SIZE), Image.LANCZOS)
        img.save(target)
        done.append(tid)
        print(f"[ok  ] {tid} → {target.name}")

    print(f"\n=== 完成 {len(done)} / 跳过 {len(skipped)} / 失败 {len(failed)} ===")
    if failed:
        print("失败:", " ".join(failed))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
