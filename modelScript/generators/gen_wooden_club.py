#!/usr/bin/env python3
"""木棍 wooden_club：一束劈开的灵木、绳缠握把。bbmodel + SML OBJ 双出。

参考图：`orthograph #11`（正 / 侧 / 背三视）。**不是一根车圆的棍**，是一**捆**劈开
的木条：下端绳缠成握把、越往上越粗、顶端是几根长短不齐的断茬。

`wooden_club`（木棍，`server/assets/items/workbench_materials.toml`）是
`plan-held-item-registration-v1 §1.2` 点名的 9 件**未注册模板**之一——server 侧
`category=weapon` / `kind=staff` / `base_attack 3.0` 的真武器，`spirit_wood ×2` 就能
在制作台搓出来，而 `BongWeaponModelRegistry` 里没有它的条目，**握在手里是空手**。

## 参考图标定（逐行像素量的，不是目测）

    前视长宽比 6.84，侧视 9.66 → **侧/正宽比 0.71**（扁束，不是圆棍）
    自顶向下逐 10% 段的平均宽（px，前视 max 157）：
        0-10%  88（断茬，个别尖端冲到 157）   50-60%  89
        10-20% 137（最粗）                    60-70%  86
        20-30% 131                            70-80%  78
        30-40% 114                            80-90%  76（绳缠段）
        40-50%  96                            90-100% 73（缠下露出的束尾）
    色（HSV，×1.25 放亮前）：断茬 38/20/41 · 木身 45/21/38 · 绳缠 46/33/32

参考图握把在**下**、断茬在**上**，与手持物约定（握把 y=0、尖端 +Y）同向，
不像骨刺那样要整个翻过来。

## 刻意偏离参考的地方

- **长宽比 6.84 → 4.4**。同小刀三件套那条：参考是实物照，MC 手持物在屏幕上只有
  三十来像素，6.8:1 渲出来是一根拖把杆。原版剑连护手约 4:1，对齐它。
- **断茬明度差从 8% 拉到 ~24%**。参考里断茬只比木身亮 8%（V41 vs V38），因为照片
  里那几根茬**互相投影**。MC 的 item 光照在这个尺度上邻接 cube 之间不投影，照抄
  8% 等于没有差别——顶端会读成"被削平的棍头"而不是"劈断的茬口"。
- **不做逐根木条的缝隙**。参考每根木条之间有 1~2px 的深缝；在 16² 的通用材质样本
  里画不出来（画了就是棋盘格，见 README「1 texel ≈ 1 单位」），改由**几何**承担：
  四根 stave 各有自己的宽度 / 前后位 / 顶高，轮廓本身是参差的。

## 为什么是「一束 stave」而不是「一根锥体」

`_stave()` 把每根木条摆在单位圆的一个 (u, v) 位上，整束按 `_taper()` 同步收放。
这样加一根、挪一根不用重算别的，而顶端断茬只要给不同的 `top` 就自然错落。
round 1 试过单根四段锥体：正视是对的，一转到侧视就是一根规整的棍，参考图那个
"捆起来的"读感全丢——那才是这件资产唯一的记忆点。

## 贴图明度未经真机标定

同 `gen_knife_trio`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`
（缺 pyrender），贴图按参考量色 ×1.25 放亮，系数是经验值不是实测。
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker.render.held_item_common import (  # noqa: E402
    Box,
    HeldItem,
    Material,
    blotch,
    build_model_json,
    build_mtl,
    build_obj,
    hand_display,
    noise_fill,
    write_assets,
)

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"
PREVIEW_DIR = Path(__file__).resolve().parents[1] / "out"

LENGTH = 0.88           # 全长（授权系，方块）
HALF_X = 0.0990         # 最粗处的半宽
HALF_Z = 0.0814         # 定成让**成束后**的侧/正宽比 = 0.71（参考实测）
GRIP = 0.150            # 拳心对准的高度：绳缠段中点
SCALE = 0.95            # 显示缩放 → 手里 0.88 × 0.95 = 0.836 方块

WRAP_LO, WRAP_HI = 0.052, 0.246     # 绳缠段
BUTT_HI = 0.032                      # 束尾切口（绳缠之下露出来的那截）


# ── 贴图 ──────────────────────────────────────────────────────────────────
# 每张 16²，OBJ 那条链**每个面整张铺满**，所以只能画通用材质样本。
# 底噪 / 斑两个原语在 `held_item_common`。


def tex_club_wood() -> Image.Image:
    """木身：风吹日晒的劈木，灰褐、纵向纤维。参考 H45 S21 V38，×1.25 放亮。

    纵纹**只画两条**、断续、低对比。这是小刀那批用三条木纹换来的教训：柄面在屏幕
    上不过十来像素，一张 16² 图铺上去超过两条竖线就必然读成条纹布。这件的"一捆"
    读感全部由几何给（四根 stave 的宽度 / 前后位 / 顶高都不同），贴图只负责材质。
    """
    rng = random.Random(0x5B01)
    img = Image.new("RGBA", (16, 16), (121, 115, 96, 255))
    noise_fill(img, rng, (121, 115, 96), 8, warm=3)
    blotch(img, rng, 4, (98, 92, 75), (2.2, 4.2))          # 泡水发黑的旧面
    blotch(img, rng, 3, (142, 135, 114), (1.8, 3.4))       # 磨亮 / 起毛处
    pixels = img.load()
    for x in (4, 11):                                      # 纵向纤维沟，断续
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = (106, 100, 83, 255)
    for _ in range(2):                                     # 顺纹的裂
        x = rng.randint(1, 14)
        for y in range(rng.randint(0, 6), rng.randint(9, 16)):
            pixels[x, y] = (84, 79, 64, 255)
    return img


def tex_club_split() -> Image.Image:
    """断茬 / 束尾切口：新劈的木质面，明显比木身亮、纹更乱。

    亮差是**刻意放大**的（参考只差 8%，这里 ~18%）——理由见模块 docstring。

    但也**不能一味放亮**：断茬占了顶部 18% 的高度，真实手持尺寸下一片冷灰的亮块
    压在深色棍身上，整件读成**火把**。所以这一档比 round 3 的 (150,140,118) 降一
    档明度、同时把色相往暖里推（R−B 从 32 到 40）——新劈开的木头本来就比风化面
    黄，暖色也让它读成"木头里面"而不是"火光"。
    """
    rng = random.Random(0x5B02)
    img = Image.new("RGBA", (16, 16), (144, 130, 104, 255))
    noise_fill(img, rng, (144, 130, 104), 9, warm=5)
    blotch(img, rng, 4, (163, 149, 122), (1.8, 3.6))       # 崭新的劈面
    blotch(img, rng, 3, (119, 107, 85), (1.6, 3.0))        # 已经起灰的
    return img


def tex_club_cord() -> Image.Image:
    """绳缠：搓过的草绳，橄榄褐、斜向拧纹。参考 H46 S33 V32。

    斜纹的方向和密度就是"绳"的全部识别特征——比木身多一档饱和度是为了在手里
    一眼分出"这段是缠着的"，参考里那圈绳也确实比木身黄得多（S33 vs S21）。
    """
    rng = random.Random(0x5B03)
    img = Image.new("RGBA", (16, 16), (102, 94, 68, 255))
    noise_fill(img, rng, (102, 94, 68), 7, warm=3)
    pixels = img.load()
    for start in range(-8, 16, 3):                         # 拧纹：暗
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (84, 77, 54, 255)
    for start in range(-6, 16, 6):                         # 受光的那面：亮
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (128, 118, 87, 255)
    return img


# ── 束形 ──────────────────────────────────────────────────────────────────


# 参考图逐 10% 段量出来的宽度剖面，换算成「占最粗处的比例」。
# 键是 y/LENGTH，值是该高度的束宽系数。
_TAPER = (
    (0.00, 0.505), (0.10, 0.513), (0.20, 0.530), (0.30, 0.550),
    (0.45, 0.648), (0.55, 0.706), (0.65, 0.806), (0.72, 0.918),
    (0.80, 0.982), (0.90, 1.000), (1.00, 0.975),
)


def _taper(y: float) -> float:
    """`y` 处的束宽系数（1.0 = 最粗）。分段线性插值 `_TAPER`。"""
    t = min(max(y / LENGTH, 0.0), 1.0)
    for (a, va), (b, vb) in zip(_TAPER, _TAPER[1:]):
        if t <= b:
            return va + (vb - va) * (t - a) / (b - a)
    return _TAPER[-1][1]


def _seg(name: str, material: str, y0: float, y1: float,
         u: float, v: float, hu: float, hv: float) -> Box:
    """一段木条。`u/v` 是它在束截面里的位置、`hu/hv` 是半宽，**都是单位系**
    （1.0 = 最粗处的半宽），乘 `_taper(段中点)` 得到真尺寸。

    单位系的意义：整束的收放只由 `_TAPER` 一处控制，挪一根木条不会牵动别的。
    """
    mid = (y0 + y1) / 2.0
    k = _taper(mid)
    return Box(name, material,
               (round(u * k * HALF_X, 5), round(mid, 5), round(v * k * HALF_Z, 5)),
               (round(hu * k * HALF_X, 5), round((y1 - y0) / 2.0, 5),
                round(hv * k * HALF_Z, 5)))


# 四根 stave 的截面排布 + 各段的 y 分界 + 顶端断茬高度。
#   u/v   截面里的位置（u 左右、v 前后；+v 朝模型背面）
#   hu/hv 半宽
#   cuts  分段的 y 分界。**五段不是四段**：round 2 用四段，正视里那条 0.57→1.0 的
#         锥度被摊成四级台阶，整根读成"一根直棍上顶了个疙瘩"。多一级就够了，
#         再多只是烧面数（收形的楼梯感在真实手持尺寸下本来就看不见）。
#   top   断茬尖到哪
# 四根的**底**也各不相同（0.024~0.035）：一是真捆起来的木条本就长短不齐，二是
# 齐平的底意味着四个 y-min 面共面且投影相交 —— `assert_no_coplanar_faces` 会拦。
# 四根的 top 刻意错开 0.795 / 0.880 / 0.845 / 0.815：参考图顶端就是**长短不齐**的，
# 齐平的顶会读成"锯断的棍"。
_STAVES = (
    # 名   u       v       hu     hv     cuts                          top
    ("core", +0.02, -0.30, 0.44, 0.46, (0.024, 0.212, 0.402, 0.590, 0.712), 0.845),
    ("back", +0.10, +0.52, 0.36, 0.34, (0.031, 0.246, 0.430, 0.624, 0.730), 0.880),
    ("left", -0.56, +0.16, 0.38, 0.40, (0.028, 0.190, 0.376, 0.558, 0.694), 0.812),
    ("right", +0.58, +0.10, 0.35, 0.38, (0.035, 0.228, 0.416, 0.606, 0.706), 0.836),
)


def _bundle(axis: int) -> tuple[float, float]:
    """整束在截面某轴上的 (中心, 半宽)，单位系。axis 0 = u（左右），1 = v（前后）。

    **算出来而不是手填**：绳缠和束尾要贴着整束的外沿走，写死一个数就等着它和
    stave 排布脱节。round 1 就是拿"包络"（±1.0）当绳缠宽度，而四根 stave 的并集
    只占 u 向 ±0.935、v 向 ±0.81——绳缠因此比木身宽 16%、深 35%，五道箍在 3/4 视
    里读成**摞起来的五片圆盘**，不是缠上去的绳。
    """
    lo = min(s[1 + axis] - s[3 + axis] for s in _STAVES)
    hi = max(s[1 + axis] + s[3 + axis] for s in _STAVES)
    return (lo + hi) / 2.0, (hi - lo) / 2.0


def part_shaft() -> list[Box]:
    """四根 stave 的主体。每根四段，段高不等——等分多段渲出来是一道楼梯
    （小刀那批实测过），这里让中段最长，收拢感被挤到顶部三成。"""
    boxes: list[Box] = []
    for name, u, v, hu, hv, cuts, top in _STAVES:
        for index, (y0, y1) in enumerate(zip(cuts, cuts[1:])):
            boxes.append(_seg(f"{name}_{index}", "club_wood", y0, y1, u, v, hu, hv))
    return boxes


def part_head() -> list[Box]:
    """顶端断茬：一簇**又高又细**、高度各不相同的木刺。

    round 2 在这里犯了本件最大的一次错：茬口做成 `hu*0.86` 宽、只有 0.05~0.08 高，
    还往外张到 `u*1.26`——四块矮而宽的盖子并排，正视直接读成**锤头 / 斧刃**，
    是"钝器"没错，但完全不是"劈开的木束"。参考图那 10% 是一簇长短不齐的**刺**：
    每根都比木条本身细得多，靠**高度差**而不是宽度差把轮廓打碎。

    所以这轮反过来：**每根刺自己必须是竖着的**——高宽比 ≥ 2。宽度收到 stave 的
    0.50~0.60、高度拉到 0.12~0.15（顶部 18% 全是刺），横向外张一路收到 `u*1.10`
    （张到 1.34 时最外那两根被甩出主体轮廓之外，3/4 视里读成"旁边飘着一块碎木"
    ——拓扑上它们其实好好连着，是**轮廓**散了不是几何断了，所以查连通性查不出来，
    只能靠看图。参差感全部交给**高度差**，横向张开只留一点点）。
    另加三根扎在缝里的细刺。逐段实心宽因此落回参考的 0.64 附近，而个别尖端仍冲出
    最粗处——参考量得的正是这个形态（段均 0.64、单点 max 1.15）。
    """
    boxes: list[Box] = []
    # 每根 stave 顶上劈出两根不等高的刺：真木条劈开是**顺纹裂成两半**，
    # 一根一个尖只会读成削尖的桩。
    for name, u, v, hu, hv, cuts, top in _STAVES:
        base = cuts[-1]
        boxes.append(_seg(f"{name}_tip", "club_split", base, top,
                          u * 1.10 - hu * 0.30, v * 1.06, hu * 0.60, hv * 0.64))
        # 同一根 stave 劈出来的第二半：**按比例**取到 62% 高，别写死一个减量——
        # 四根 stave 的 tip 长度差着一倍（0.06~0.10），写死会让最短那根算出负高度。
        boxes.append(_seg(f"{name}_tip2", "club_split", base + 0.013,
                          base + (top - base) * 0.74,
                          u * 1.10 + hu * 0.34, v * 1.06 + hv * 0.20,
                          hu * 0.50, hv * 0.54))
    # 三根扎在束心缝里的细刺，高度全不同。
    boxes.append(_seg("splinter_a", "club_split", 0.722, 0.868, -0.21, -0.05, 0.17, 0.19))
    boxes.append(_seg("splinter_b", "club_split", 0.700, 0.822, +0.36, +0.33, 0.15, 0.17))
    boxes.append(_seg("splinter_c", "club_split", 0.736, 0.840, -0.06, +0.48, 0.14, 0.16))
    return boxes


def part_wrap() -> list[Box]:
    """绳缠五道，贴着整束外沿绕上去。

    厚薄与前后位**刻意参差**：等厚等宽的几道会读成"车出来的凹槽"，参差才读成一圈圈
    绕上去的绳（小刀那批同一条经验）。`proud` 是相对整束外沿的外凸量，1.03 = 只鼓出
    3%——参考图里那圈绳几乎与木身齐平，鼓多了就是五片圆盘。
    """
    cu, hu = _bundle(0)
    cv, hv = _bundle(1)
    coils = (
        # y0      y1      proud   u 偏     v 偏
        (0.0545, 0.0855, 1.038, -0.030, +0.022),
        (0.0925, 0.1225, 1.022, +0.034, -0.018),
        (0.1300, 0.1615, 1.044, -0.026, +0.030),
        (0.1690, 0.1980, 1.019, +0.030, -0.024),
        (0.2055, 0.2380, 1.031, -0.034, +0.016),
    )
    boxes = []
    for index, (y0, y1, proud, du, dv) in enumerate(coils):
        boxes.append(_seg(f"wrap_{index}", "club_cord", y0, y1,
                          cu + du, cv + dv, hu * proud, hv * proud))
    return boxes


def part_butt() -> list[Box]:
    """束尾：绳缠之下露出来的那截切口，看得见每根木条的断面。"""
    cu, hu = _bundle(0)
    cv, hv = _bundle(1)
    # 材质是 **club_wood** 不是 club_split：round 3 用亮色，正视里束尾成了一只白脚，
    # 把视线从断茬那头拽下来。切口的"新木"读感在真实手持尺寸下根本看不见（这一截
    # 只有全长的 3%），而错误的亮块在任何尺寸下都显眼。
    return [_seg("butt", "club_wood", 0.0, BUTT_HI, cu, cv, hu * 0.96, hv * 0.95)]


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_butt() + part_wrap() + part_shaft() + part_head())


WOODEN_CLUB = HeldItem(
    key="wooden_club",
    display_name="木棍",
    # 宿主一栏填 stick 只是**登记意图**，`--install` 会拒绝：`item/stick` 现在被
    # poison_needle 白嫖（`BongWeaponModelRegistry`），劫持它等于把毒针变成木棍。
    host_item="stick",
    boxes=all_boxes(),
    materials=(
        Material("club_wood", (0.47, 0.45, 0.38), tex_club_wood()),
        Material("club_split", (0.59, 0.55, 0.46), tex_club_split()),
        Material("club_cord", (0.40, 0.37, 0.27), tex_club_cord()),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (WOODEN_CLUB,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel，不出图")
    parser.add_argument("--install", action="store_true",
                        help="【当前会直接报错】写进 client 资源树，见下方拒绝理由")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true",
                        help="model JSON（含 display 变换）打到 stdout。装不了机时，"
                             "这是拿到 display 块的唯一途径——`preview_player_anim.py "
                             "--display` 要它才能把棍摆到手上：\n"
                             "  python3 modelScript/generators/gen_wooden_club.py "
                             "--dump-display > /tmp/club.json")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(WOODEN_CLUB), end="")
        return

    if args.dump_obj:
        print(build_obj(WOODEN_CLUB))
        print(build_mtl(WOODEN_CLUB))
        return

    if args.install:
        # 和小刀三件套同一堵墙：宿主机制的粒度是「一个 vanilla item → 一份 model
        # JSON」，而 `item/stick` 已经被 `poison_needle`（毒针）白嫖着当原版模型用。
        # 劫持它 = 全服毒针长成木棍，且 diff 里只是一份 JSON 变了、看不出牵连。
        # 换个冷门 vanilla item 只是把问题推后一件——剩下 22 件借皮的没那么多冷门
        # item 可烧。用户 2026-08-24 已裁决根治。
        raise SystemExit(
            "拒绝 --install：宿主 item/stick 已被 poison_needle（毒针）占用，"
            "劫持它会把毒针一起变成木棍。\n"
            "装机前先落地 docs/plans-skeleton/plan-held-item-registration-v1.md，"
            "让每个模板注册自己的 render-only Item。"
        )

    outputs = write_assets(
        items(),
        bbmodel_dir=BBMODEL_DIR,
        client_resources=None,      # 见 --install 那段：现在装不了
        preview_dir=PREVIEW_DIR,
        render_previews=not args.no_preview,
    )
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
