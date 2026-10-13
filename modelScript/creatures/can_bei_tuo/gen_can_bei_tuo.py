#!/usr/bin/env python3
"""残背驮兽（can_bei_tuo）Blockbench .bbmodel 生成器 —— Round 3/3。

几何全部在同目录 can_bei_tuo_parts.py 里（按部件拆成 part_*()）；本文件负责：
装配 Rig、门禁（gatekit）、逐件预览、完整模型、六视角接触表 check.png。

只做几何：不贴图（调色板色块）、不做动画、不接线、不写 manifest。

用法（需要 bong-modelscript 虚拟环境，里面装着 bbmodel_maker 的渲染与门禁）：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --self-test
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --part head
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo")
PARTS_DIR = REVIEW_DIR / "parts"

sys.path.insert(0, str(CREATURE_DIR))
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402
from cjk_font import load_font  # noqa: E402

import can_bei_tuo_parts as parts  # noqa: E402

ROUND = 3
PREV_ROUND = 2
# Round 2 的数字，PREV 栏标题用（Round 2 的 bbmodel 存在 REVIEW_DIR/CanBeiTuo_round2.bbmodel）
PREV_CUBES, PREV_BONES = 225, 41
BONE_FLOOR = 32  # 任务卡：四足兽 ≥ 32 骨


class CanBeiTuoRig(Rig):
    """rigkit.Rig + 按「族」写 element.color。

    rigkit 默认把材质序号 % 8 写进 color，13 种材质会撞色，门禁就分不清谁是谁。
    这里改成：同族材质（三种皮、三种藓…）共用一个 color，门禁按族判断软覆盖。
    """

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.cube_mats: dict[str, str] = {}  # element uuid -> parts.MATS 里的材质名，贴图版用

    def cube(self, bone, name, frm, to, *, rot=None, org=None, mat="hide_base"):
        el = super().cube(bone, name, frm, to, rot=rot, org=org, mat=mat)
        el["color"] = parts.FAMILY_ORDER.index(parts.FAMILY[mat])
        self.cube_mats[el["uuid"]] = mat
        return el


def new_rig() -> CanBeiTuoRig:
    rig = CanBeiTuoRig(parts.MATS, tex=64, swatch=8)
    for name, parent, pivot, _ in parts.BONES_DEF:
        rig.bone(name, pivot, parent=parent)
    return rig


def build_rig(part: str = "all") -> CanBeiTuoRig:
    rig = new_rig()
    if part == "all":
        parts.build_all(rig)
    else:
        for fn in parts.PARTS_META[part][1]:
            fn(rig)
    return rig


# ──────────────────────────────────────────────────────────────── 门禁
class CreatureGates(gatekit.AssetGates):
    """去掉「越出 0..16 方块」一道：这是 GeckoLib 实体模型，不是放进方块里的东西。"""

    def specs(self):
        return tuple(s for s in super().specs() if s[0] != "overflow")


# 软覆盖：这些族对之间的跨骨重叠是设计意图（藓盖在皮上、褶皱压在皮上、角嵌进颅骨…）
_PAIRS = [("moss", "hide"), ("fold", "hide"), ("horn", "hide"), ("hoof", "hide"), ("socket", "hide"),
          ("eye", "socket"), ("flower", "moss"), ("fold", "moss"), ("moss", "hoof"), ("fold", "hoof"),
          ("horn", "moss"), ("fold", "horn"), ("socket", "horn"), ("eye", "hide"), ("horn", "socket")]
GATES = CreatureGates(
    "残背驮兽 / can_bei_tuo",
    dict.fromkeys(parts.FAMILY_ORDER, (0, 0, 0)),
    # 藓骨上的须 / 花是随机撒的，左右不要求镜像
    asym=tuple(n for n, *_ in parts.BONES_DEF if n.startswith("moss_") or n == "neck_moss_dewlap"),
    # 藓花、悬挂的垂须尖本来就是浮在藓上的小件
    soft_over=frozenset(frozenset(p) for p in _PAIRS),
)


def count_bones_with_geometry(rig: Rig) -> int:
    return sum(1 for n, _, _, _ in parts.BONES_DEF if n != "root" and rig.bones[n]["children"])


def self_test() -> None:
    """门禁差分自证：干净模型必须 0 违例，注入缺陷后每道门必须报得出来。"""
    rig = build_rig("all")
    broken = GATES.self_test(rig)
    if broken:
        raise SystemExit(f"{broken} 道门失效（注入缺陷却报不出）")
    total = GATES.total(rig)
    if total:
        GATES.report(rig, px=16.0)
        raise SystemExit(f"干净模型仍有 {total} 处违例")
    low = min(e["from"][1] for e in rig.elements)
    assert abs(low) < 1e-4, f"没贴地：最低点 y={low}"
    n_bones = len(parts.BONES_DEF)
    assert n_bones >= BONE_FLOOR, f"骨骼不足：{n_bones} < {BONE_FLOOR}"
    empty = [n for n, *_ in parts.BONES_DEF if n != "root" and not rig.bones[n]["children"]]
    assert not empty, f"这些骨没有几何（不能算可动关节）：{empty}"
    print(f"✓ 自证通过：{len(rig.elements)} cubes / {n_bones} 骨（其中 {count_bones_with_geometry(rig)} 根挂几何）")


# ──────────────────────────────────────────────────────────────── 渲染与审阅图
BG = (119, 119, 119)
VIEWS = [
    # (key, yaw, pitch, 标签) —— 标签写出实际照到的轴面。
    # 渲染器的 yaw=0 照的是 +Z 面（Round 2 把它叫 FRONT，渲出来却是臀和尾），
    # 头在 -Z，所以正视是 yaw=180；yaw=-90 的相机在 +X 侧。逐个用「头朝哪」核过。
    ("FRONT", 180.0, 0.0, "正视 FRONT（照 -Z 面：吻/双角/前腿）"),
    ("ISO", 145.0, 22.0, "3/4 俯视 ISO（-Z 前面 + -X 左侧 + 顶面）"),
    ("SIDE_L", 90.0, 0.0, "左视 SIDE_L（照 -X 面）"),
    ("SIDE_R", -90.0, 0.0, "右视 SIDE_R（照 +X 面）"),
    ("TOP", 0.0, 89.9, "俯视 TOP（照 +Y 面，头在上）"),
    ("BACK", 0.0, 0.0, "后视 BACK（照 +Z 面：臀/尾/后腿）"),
]


def render_six_views(bbmodel_path: Path, title: str, cell: int = 400) -> tuple[Image.Image, dict[str, Image.Image]]:
    shots = {key: render(bbmodel_path, yaw=yaw, pitch=pitch, size=cell, bg=BG)[0] for key, yaw, pitch, _ in VIEWS}
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 56), (32, 33, 36))
    draw = ImageDraw.Draw(sheet)
    draw.text((10, 6), title, fill=(235, 235, 235), font=load_font(18))
    font_label = load_font(13)
    for i, (key, _, _, label) in enumerate(VIEWS):
        px = 5 + (i % 3) * (cell + 5)
        py = 34 + (i // 3) * (cell + 10)
        sheet.paste(shots[key], (px, py))
        draw.rectangle([px, py, px + 330, py + 24], fill=(22, 23, 26))
        draw.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_label)
    return sheet, shots


def bone_tree_lines(rig: Rig) -> list[str]:
    kids: dict[str | None, list[str]] = {}
    for name, parent, _, _ in parts.BONES_DEF:
        kids.setdefault(parent, []).append(name)
    lines: list[str] = []

    def walk(name: str, depth: int) -> None:
        n = len(rig.bones[name]["children"])
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name}  [{n}]")
        for k in kids.get(name, []):
            walk(k, depth + 1)

    walk("root", 0)
    return lines


def render_bone_tree(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    step = 15
    h = max(height, 96 + len(lines) * step)
    panel = Image.new("RGB", (400, h), (32, 33, 37))
    draw = ImageDraw.Draw(panel)
    draw.rectangle([0, 0, 399, h - 1], outline=(70, 75, 85), width=2)
    draw.text((12, 8), "骨骼层级树（方括号 = 该骨挂的 cube 数）", fill=(180, 235, 200), font=load_font(15))
    draw.text((12, 32), f"共 {len(parts.BONES_DEF)} 骨（含 root），{count_bones_with_geometry(rig)} 根挂几何；下限 {BONE_FLOOR}",
              fill=(240, 210, 150), font=load_font(13))
    draw.text((12, 52), f"cube 总数 {len(rig.elements)}（Round {PREV_ROUND}：{PREV_CUBES}）",
              fill=(190, 210, 235), font=load_font(13))
    font = load_font(11)
    y = 78
    for ln in lines:
        draw.text((12, y), ln, fill=(230, 230, 235), font=font)
        y += step
    return panel


def render_part_previews() -> None:
    """逐件单独预览：每个部件一张三视图，确认单件无误后才拼接。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    views = [("正视 FRONT（照 -Z 面）", 180.0, 0.0), ("3/4 ISO（-Z 前 / -X 左 / 顶）", 145.0, 22.0),
             ("右视 SIDE_R（照 +X 面）", -90.0, 0.0)]
    for idx, (key, (desc, _)) in enumerate(parts.PARTS_META.items(), start=1):
        rig = build_rig(key)
        tmp.write_text(json.dumps(rig.bbmodel(f"CanBeiTuo_part_{key}")), encoding="utf-8")
        canvas = Image.new("RGB", (300 * 3 + 20, 300 + 52), (32, 33, 36))
        draw = ImageDraw.Draw(canvas)
        draw.text((10, 6), f"部件 {idx}: {desc}（{len(rig.elements)} cubes）", fill=(235, 235, 235), font=load_font(16))
        for i, (label, yaw, pitch) in enumerate(views):
            canvas.paste(render(tmp, yaw=yaw, pitch=pitch, size=300, bg=BG)[0], (5 + i * 305, 32))
            draw.text((10 + i * 305, 336), label, fill=(200, 200, 200), font=load_font(13))
        canvas.save(PARTS_DIR / f"part_{idx}_{key}.png")
        print(f"  部件 {idx} {key}: {len(rig.elements)} cubes")
    tmp.unlink(missing_ok=True)


def make_check_sheet(sheet_prev: Image.Image, sheet_now: Image.Image, tree: Image.Image, n_cubes: int, n_bones: int) -> Image.Image:
    ref = Image.open(REVIEW_DIR / "can_bei_tuo_ref.png").convert("RGB")
    exp_path = REVIEW_DIR / "can_bei_tuo_exploded.png"
    exp = Image.open(exp_path).convert("RGB") if exp_path.exists() else ref
    row_h = max(sheet_now.height, tree.height)
    gap = 12
    sub_h = (row_h - gap) // 2
    sub_w = int(ref.width * sub_h / ref.height)
    total_w = sub_w + sheet_prev.width + sheet_now.width + tree.width + 50
    canvas = Image.new("RGB", (total_w, row_h + 48), (25, 26, 29))
    draw = ImageDraw.Draw(canvas)
    head = load_font(18)
    x = 10
    draw.text((x + 4, 8), "参考图（上：整兽 / 下：爆炸）", fill=(215, 205, 180), font=head)
    canvas.paste(ref.resize((sub_w, sub_h), Image.Resampling.LANCZOS), (x, 38))
    canvas.paste(exp.resize((sub_w, sub_h), Image.Resampling.LANCZOS), (x, 38 + sub_h + gap))
    x += sub_w + 10
    draw.text((x + 4, 8), f"PREV：Round {PREV_ROUND}（{PREV_CUBES} cubes / {PREV_BONES} 骨），同取景", fill=(230, 160, 140), font=head)
    canvas.paste(sheet_prev, (x, 38))
    x += sheet_prev.width + 10
    draw.text((x + 4, 8), f"NOW：Round {ROUND}（{n_cubes} cubes / {n_bones} 骨）", fill=(140, 230, 180), font=head)
    canvas.paste(sheet_now, (x, 38))
    x += sheet_now.width + 10
    draw.text((x + 4, 8), "骨骼层级树", fill=(180, 220, 240), font=head)
    canvas.paste(tree, (x, 38))
    return canvas


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--part", choices=["all", *parts.PARTS_META], default="all", help="只生成某一部件的 bbmodel")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁差分自证")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return
    self_test()
    GATES.report(build_rig("all"), px=16.0)

    CREATURE_DIR.mkdir(parents=True, exist_ok=True)
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    if args.part != "all":
        rig = build_rig(args.part)
        out = CREATURE_DIR / f"CanBeiTuo_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"CanBeiTuo_{args.part}"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cubes）")
        return

    # 先逐件预览，再拼接
    render_part_previews()
    rig = build_rig("all")
    text = json.dumps(rig.bbmodel("CanBeiTuo"), ensure_ascii=False, indent=2)
    out = CREATURE_DIR / "CanBeiTuo.bbmodel"
    out.write_text(text, encoding="utf-8")
    (MODELS_DIR / "CanBeiTuo.bbmodel").write_text(text, encoding="utf-8")
    n_cubes, n_bones = len(rig.elements), len(parts.BONES_DEF)
    print(f"✓ 完整模型 {out}（{n_cubes} cubes / {n_bones} 骨）")

    sheet_now, shots = render_six_views(out, f"can_bei_tuo 残背驮兽 Round {ROUND} | {n_cubes} cubes | {n_bones} 骨")
    sheet_now.save(REVIEW_DIR / "contact_sheet.png")
    (REVIEW_DIR / "views").mkdir(exist_ok=True)
    for key, im in shots.items():
        im.save(REVIEW_DIR / "views" / f"{key.lower()}.png")

    prev_model = REVIEW_DIR / f"CanBeiTuo_round{PREV_ROUND}.bbmodel"
    sheet_prev, _ = render_six_views(prev_model, f"Round {PREV_ROUND}（{PREV_CUBES} cubes / {PREV_BONES} 骨）")
    sheet_prev.save(REVIEW_DIR / f"contact_sheet_round{PREV_ROUND}.png")

    tree = render_bone_tree(rig, sheet_now.height)
    tree.save(REVIEW_DIR / "bones_tree.png")
    check = make_check_sheet(sheet_prev, sheet_now, tree, n_cubes, n_bones)
    check.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png {check.size}")


if __name__ == "__main__":
    main()
