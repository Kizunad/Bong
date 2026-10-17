#!/usr/bin/env python3
"""铁锁与钥（tie_suo_yue）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #6（手持 / 佩挂小件）。
世界观基调：
- 精致规整的古代铸铁弹簧锁：生铁铸造卧牛形锁身（头颈微昂、牛角后伏、隆起脊背与前胸、腹底收敛、盘卧四蹄与卷尾），锻打生铁 U 形锁梁，侧面外露一小段双叉青铜簧片。
- 锁面浅浮雕一圈规整阴刻缠枝忍冬纹带。
- 独立细钥：一把长约 4–5 px 的锻铁钥匙，首端为镂空如意方圈，带三阶顶簧齿板，通过细麻绳拴在锁身旁。
- 末法体现：锁身侧面留有一处长方形凹槽（原本嵌灵气符骨的阵片位），如今空无一物，只剩纯物理机械锁死，不再具备自毁灵阵。
- 尺寸：锁身约 5×7×2.6 px，钥匙约 4–5 px 长，整体黑褐氧化膜，锁梁与钥匙孔有手抚磨亮包浆。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/metalware/tie_suo_yue/gen_tie_suo_yue.py --self-test
    python3 modelScript/metalware/tie_suo_yue/gen_tie_suo_yue.py --gates
    python3 modelScript/metalware/tie_suo_yue/gen_tie_suo_yue.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "gear" / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "iron":         (72, 68, 66),      # 铸铁黑褐氧化皮（卧牛锁身底色）
    "iron_lt":      (124, 120, 118),   # 锁梁手摸磨光面、角尖与钥匙棱光
    "iron_dk":      (48, 44, 42),      # 阴刻纹槽底与接缝暗铁
    "rust":         (146, 76, 46),     # 锁面角落暗红铁锈
    "rust_dk":      (98, 48, 30),      # 锈坑与陈年旧蚀
    "bronze":       (148, 112, 68),    # 外露青铜弹簧片与定位铆件
    "bronze_lt":    (188, 148, 98),    # 簧片磨亮插拔接触面
    "cord":         (146, 126, 82),    # 拴钥细麻绳
    "cord_dk":      (104, 88, 56),     # 麻绳打结暗色
    "void":         (18, 16, 14),      # 空置阵位、锁孔狭缝与钥匙圈心
}

SOFT = ("cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("lock_casing", "root", (0.0, 0.0, 0.0), "卧牛锁身：隆起牛脊背、微凹腰腹与圆浑臀部（对称）"),
    ("ox_head", "lock_casing", (0.0, 1.2, -3.2), "卧牛头颈部：昂起牛首、方阔吻鼻、牛眼眶与鼻环（对称）"),
    ("ox_horns", "ox_head", (0.0, 1.8, -2.6), "后伏双角与耳：向后贴附的起脊弯角与小立耳（对称）"),
    ("ox_legs", "lock_casing", (0.0, -1.5, 0.0), "盘卧四蹄与地台：收拢在身下的四肢卧足与支撑台（对称）"),
    ("ox_tail", "lock_casing", (0.4, 0.0, 2.8), "盘臀尾须：贴后臀侧折贴的牛尾根与尾鬃（不对称）"),
    ("shackle_u", "lock_casing", (0.0, 2.2, 0.0), "锻打U形锁梁：滑动锁梁长横杆、前插鼻与后锁栓（对称）"),
    ("vine_relief", "lock_casing", (0.0, 0.0, 0.0), "阴刻缠枝纹：锁身两侧规整忍冬卷草浅浮雕饰带（对称）"),
    ("keyhole_plate", "lock_casing", (0.0, -0.8, 3.4), "锁尾钥匙插孔：后臀平直开槽钥匙孔与锁芯导向板（对称）"),
    ("bronze_spring", "lock_casing", (-1.25, -0.6, -0.2), "外露铜簧片：侧下缝隙露出的双叉青铜弹簧片（不对称）"),
    ("rune_slot", "lock_casing", (1.25, 0.2, 0.4), "末法空置阵片位：右侧原本嵌灵气符骨的方形内凹暗槽（不对称）"),
    ("tie_cord", "shackle_u", (-1.8, 2.2, 0.0), "拴钥细麻绳：从锁梁左侧系紧下垂连接钥匙的编绳（挂件独立骨）"),
    ("key_bow", "tie_cord", (-2.6, 0.8, 0.0), "钥匙方圈首：镂空如意方圈匙纽与打孔绳耳（挂件独立骨）"),
    ("key_stem", "key_bow", (-2.6, -1.0, 0.0), "细长钥匙杆：锻铁阶梯圆杆与定位环挡（挂件独立骨）"),
    ("key_bit", "key_stem", (-2.6, -2.6, 0.0), "三阶开簧齿板：端部精密开槽齿牙，用于压合铜簧（挂件独立骨）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in (
    "root", "lock_casing", "ox_head", "ox_horns", "ox_legs", "shackle_u", "vine_relief", "keyhole_plate"
))


# ───────────────────────────────────────────────────────────── 零件建模实现
def part_lock_casing(g: Gear) -> None:
    """卧牛锁身：隆起牛脊背、微凹腰腹与圆浑臀部，带前后穿梁套筒（严格对称于 X=0）。"""
    b = "lock_casing"
    # 锁身主箱体（厚实躯干）
    g.box(b, "body_core_mid", (-1.16, -1.20, -2.80), (1.16, 1.30, 2.80), "iron")
    g.box(b, "body_core_bot", (-0.95, -1.45, -2.60), (0.95, -1.15, 2.60), "iron_dk")

    # 隆起肩胛与牛脊梁：前肩峰镂空套筒（左壁、右壁、顶拱扣，中间留出穿梁孔）
    g.pair(b, "spine_shoulder_wall", (0.42, 1.25, -2.50), (0.75, 2.72, -1.25), "iron")
    g.box(b, "spine_shoulder_crest", (-0.55, 2.78, -2.40), (0.55, 3.05, -1.35), "iron_lt")
    g.pair(b, "spine_shoulder_brace", (0.72, 1.45, -2.35), (1.15, 2.45, -1.45), "iron_lt")

    # 中部马鞍形微凹腰背（锁梁紧贴鞍背穿过）
    g.box(b, "spine_saddle_mid", (-0.65, 1.20, -1.25), (0.65, 1.95, 0.95), "iron")
    g.box(b, "spine_saddle_ridge", (-0.25, 1.92, -1.15), (0.25, 2.12, 0.85), "iron_lt")

    # 后臀隆起峰：后臀镂空套筒（左壁、右壁、顶拱扣，中间留出穿梁孔）
    g.pair(b, "spine_rump_wall", (0.42, 1.25, 0.95), (0.75, 2.72, 2.50), "iron")
    g.box(b, "spine_rump_crest", (-0.55, 2.78, 1.05), (0.55, 3.05, 2.40), "iron_lt")
    g.pair(b, "spine_rump_brace", (0.72, 1.45, 1.05), (1.15, 2.45, 2.40), "iron_lt")

    # 两侧胸胁丰满鼓肉（成对镜像）
    g.pair(b, "body_flank_shoulder", (0.95, -0.90, -2.48), (1.35, 1.10, -0.85), "iron")
    g.pair(b, "body_flank_waist", (0.85, -0.80, -0.85), (1.20, 1.00, 0.85), "iron")
    g.pair(b, "body_flank_rump", (0.95, -0.90, 0.85), (1.35, 1.10, 2.50), "iron")

    # 前胸饱满圆凸胸阜
    g.box(b, "body_chest_front", (-0.95, -1.10, -3.20), (0.95, 1.00, -2.75), "iron")
    g.box(b, "body_chest_keel", (-0.25, -0.90, -3.32), (0.25, 0.80, -3.15), "iron_lt")
    g.pair(b, "body_chest_flange", (0.55, -0.75, -3.25), (0.85, 0.65, -2.95), "iron_lt")

    # 后臀端立面
    g.box(b, "body_rear_wall", (-1.00, -1.10, 2.75), (1.00, 1.10, 3.28), "iron")
    g.pair(b, "body_rear_bracket", (0.65, -0.85, 2.85), (0.95, 0.85, 3.25), "iron_lt")


def part_ox_head(g: Gear) -> None:
    """卧牛头颈部：昂起牛首、方阔吻鼻、牛眼眶与鼻环（严格对称）。"""
    b = "ox_head"
    # 颈项上斜过渡（强壮牛颈）
    g.box(b, "ox_neck_slope", (-0.85, 0.65, -3.40), (0.85, 1.62, -2.45), "iron")
    g.box(b, "ox_dewlap_throat", (-0.45, 0.15, -3.50), (0.45, 0.85, -2.65), "iron_dk")
    g.box(b, "ox_neck_crest", (-0.35, 1.45, -3.22), (0.35, 1.82, -2.35), "iron_lt")

    # 方面阔吻鼻台（典型的方阔牛吻）
    g.box(b, "ox_muzzle_main", (-0.95, 0.35, -4.35), (0.95, 1.32, -3.30), "iron")
    g.box(b, "ox_nose_bridge", (-0.45, 1.25, -4.20), (0.45, 1.55, -3.25), "iron_lt")
    g.box(b, "ox_lip_jaw", (-0.75, -0.15, -4.15), (0.75, 0.45, -3.25), "iron_dk")

    # 双鼻孔与悬挂生铁小鼻环
    g.pair(b, "ox_nostril", (0.28, 0.45, -4.42), (0.58, 0.85, -4.25), "void")
    g.box(b, "ox_nosering_t", (-0.35, 0.12, -4.40), (0.35, 0.38, -4.05), "iron_lt")
    g.box(b, "ox_nosering_b", (-0.35, -0.38, -4.35), (0.35, -0.05, -3.95), "iron_lt")

    # 牛眼眶与眼珠凸起（成对对称，错开 Y 轴）
    g.pair(b, "ox_eye_brow", (0.78, 1.25, -3.45), (1.15, 1.68, -2.75), "iron_lt")
    g.pair(b, "ox_eye_ball", (0.82, 0.98, -3.35), (1.12, 1.38, -2.85), "iron_lt")
    g.pair(b, "ox_eye_socket", (0.75, 0.85, -3.38), (1.08, 1.40, -2.70), "iron_dk")

    # 额前如意纹雕饰（增补细节）
    g.box(b, "ox_brow_ruyi_mid", (-0.22, 1.52, -3.65), (0.22, 1.82, -3.22), "iron_lt")
    g.pair(b, "ox_brow_ruyi_curl", (0.22, 1.55, -3.55), (0.48, 1.78, -3.32), "iron_lt")


def part_ox_horns(g: Gear) -> None:
    """后伏双角与小立耳：张扬雄浑的大月牙生铁牛角与机警小牛耳（成对对称，牛形核心标志！）。"""
    b = "ox_horns"
    # 额顶基座与额板（Z 上限收于 -2.40 彻底消除共面）
    g.box(b, "ox_forehead_crest", (-0.52, 1.55, -3.20), (0.52, 2.05, -2.40), "iron_lt")

    # 1. 粗壮角根（从额顶向两侧外拔，明显横向展开）
    g.pair(b, "horn_root_base", (0.52, 1.65, -3.05), (1.25, 2.25, -2.35), "iron")
    g.pair(b, "horn_root_rib", (0.62, 1.85, -2.95), (1.18, 2.35, -2.25), "iron_lt")
    g.pair(b, "horn_root_collar", (0.72, 1.72, -2.85), (1.12, 2.15, -2.45), "bronze")

    # 2. 弧曲角身（向上向斜后弧形拱起）
    g.pair(b, "horn_mid_curve", (1.15, 2.05, -2.65), (1.82, 2.85, -1.75), "iron")
    g.pair(b, "horn_mid_ridge", (1.25, 2.25, -2.55), (1.75, 2.95, -1.65), "iron_lt")
    g.pair(b, "horn_mid_ferrule", (1.35, 2.35, -2.40), (1.78, 2.75, -1.95), "iron_dk")

    # 3. 尖锐角梢（向后内敛弯尖，磨光锋利）
    g.pair(b, "horn_tip_taper", (1.22, 2.45, -1.85), (1.68, 3.12, -0.95), "iron_lt")
    g.pair(b, "horn_tip_point", (0.92, 2.35, -1.15), (1.42, 2.92, -0.45), "iron_lt")
    g.pair(b, "horn_tip_claw", (0.78, 2.45, -0.65), (1.18, 2.75, -0.25), "iron_lt")

    # 角下机警小牛耳
    g.pair(b, "ox_ear_main", (0.92, 1.05, -2.45), (1.52, 1.55, -1.95), "iron")
    g.pair(b, "ox_ear_hollow", (1.02, 1.15, -2.35), (1.42, 1.45, -2.05), "iron_dk")


def part_ox_legs(g: Gear) -> None:
    """盘卧四条短粗短腿：锁身两侧清晰可辨的四肢短腿与蹄壳（成对对称，一眼看清四腿！）。"""
    b = "ox_legs"
    # 1. 前左/前右短腿（向斜前方趴卧，外露明显）
    # 大臂/肘关节（凸出在身侧 x=±1.1..1.65, y=-0.5..0.4, z=-2.4..-1.2）
    g.pair(b, "foreleg_shoulder", (1.10, -0.45, -2.35), (1.65, 0.45, -1.25), "iron")
    # 小腿折叠段（前伸落地）
    g.pair(b, "foreleg_shank", (1.15, -1.25, -2.55), (1.68, -0.35, -1.45), "iron")
    # 前蹄壳（双瓣蹄尖着地 y=-1.5，in内瓣与out外瓣严格成对）
    g.pair(b, "foreleg_hoof_in", (1.08, -1.55, -2.65), (1.38, -1.15, -2.05), "iron_lt")
    g.pair(b, "foreleg_hoof_out", (1.42, -1.55, -2.65), (1.72, -1.15, -2.05), "iron_lt")
    g.pair(b, "foreleg_hoof_cleft", (1.32, -1.52, -2.68), (1.48, -1.18, -2.25), "iron_dk")
    g.pair(b, "foreleg_knee_stud", (1.25, -0.15, -2.42), (1.55, 0.15, -2.15), "iron_lt")

    # 2. 后左/后右短腿（向后侧盘曲，大腿肉丘饱满外鼓）
    # 丰满臀髋与大腿丘（凸出在身侧 x=±1.1..1.72, y=-0.4..0.6, z=0.8..2.2）
    g.pair(b, "hindleg_hip", (1.10, -0.35, 0.78), (1.72, 0.55, 2.15), "iron")
    # 后小腿反折向前
    g.pair(b, "hindleg_shank", (1.15, -1.25, 0.55), (1.68, -0.25, 1.65), "iron")
    # 后蹄壳（双瓣蹄尖着地 y=-1.5，in内瓣与out外瓣严格成对）
    g.pair(b, "hindleg_hoof_in", (1.08, -1.55, 0.35), (1.38, -1.15, 0.95), "iron_lt")
    g.pair(b, "hindleg_hoof_out", (1.42, -1.55, 0.35), (1.72, -1.15, 0.95), "iron_lt")
    g.pair(b, "hindleg_hoof_cleft", (1.32, -1.52, 0.32), (1.48, -1.18, 0.75), "iron_dk")
    g.pair(b, "hindleg_hock_stud", (1.25, -0.15, 1.75), (1.55, 0.15, 2.05), "iron_lt")

    # 3. 腹底卧地支撑基座（微内缩错开 body_core_bot，消除共面）
    g.box(b, "belly_ground_rest", (-0.88, -1.55, -2.25), (0.88, -1.35, 2.25), "iron_dk")


def part_ox_tail(g: Gear) -> None:
    """盘臀尾须：贴后臀侧折贴的牛尾根与尾鬃（不对称）。"""
    b = "ox_tail"
    g.box(b, "ox_tail_root", (-0.18, 0.40, 2.80), (0.22, 1.00, 3.20), "iron_dk")
    g.box(b, "ox_tail_curl_r", (0.18, 0.05, 2.85), (0.62, 0.60, 3.20), "iron_lt")
    g.box(b, "ox_tail_tip_tuft", (0.50, -0.50, 2.80), (0.90, 0.20, 3.15), "iron_lt")
    g.box(b, "ox_tail_hair_fray", (0.62, -0.82, 2.82), (0.85, -0.40, 3.12), "iron_dk")


def part_shackle_u(g: Gear) -> None:
    """锻打U形锁梁：锁梁从牛背鞍部套筒穿插贯过，前后直角弯折插进体腔（严格对称于 X=0）。"""
    b = "shackle_u"
    # 1. 顶部贯通长主梁（横跨牛背鞍部，贯穿前后肩臀套筒，紧贴牛背鞍面穿过！）
    g.box(b, "shackle_top_bar", (-0.38, 2.15, -2.60), (0.38, 2.65, 2.60), "iron_lt")
    g.box(b, "shackle_top_crest", (-0.22, 2.62, -2.45), (0.22, 2.88, 2.45), "iron_lt")

    # 锁梁锻打锤痕包浆与加固竹节箍
    g.box(b, "shackle_hammer1", (-0.42, 2.22, -1.25), (0.42, 2.55, -0.45), "iron")
    g.box(b, "shackle_hammer2", (-0.42, 2.22, 0.55), (0.42, 2.55, 1.35), "iron")
    g.box(b, "shackle_bamboo_rib1", (-0.45, 2.12, -0.95), (0.45, 2.70, -0.75), "iron_lt")
    g.box(b, "shackle_bamboo_rib2", (-0.45, 2.12, 0.85), (0.45, 2.70, 1.05), "iron_lt")

    # 2. 前端直角弯折插鼻（宽设为 0.35 错开 0.38 消除共面）
    g.box(b, "shackle_front_curve", (-0.35, 1.85, -2.92), (0.35, 2.62, -2.45), "iron_lt")
    g.box(b, "shackle_front_leg", (-0.32, 0.75, -2.90), (0.32, 1.95, -2.38), "iron_lt")
    g.box(b, "shackle_front_collar", (-0.48, 1.20, -3.02), (0.48, 1.68, -2.28), "iron_dk")

    # 3. 后端直角弯折锁柱（宽设为 0.35 错开 0.38 消除共面）
    g.box(b, "shackle_rear_curve", (-0.35, 1.85, 2.45), (0.35, 2.62, 2.92), "iron_lt")
    g.box(b, "shackle_rear_leg", (-0.32, 0.75, 2.38), (0.32, 1.95, 2.90), "iron_lt")
    g.box(b, "shackle_rear_collar", (-0.48, 1.20, 2.28), (0.48, 1.68, 3.02), "iron_dk")

    # 磨光包浆凸面
    g.box(b, "shackle_wear_spot1", (-0.45, 2.28, -0.15), (0.45, 2.62, 0.35), "iron_lt")
    g.box(b, "shackle_stop_notch", (-0.25, 1.78, -2.95), (0.25, 2.02, -2.75), "iron_dk")


def part_vine_relief(g: Gear) -> None:
    """阴刻缠枝纹：锁身两侧规整忍冬卷草浅浮雕饰带（成对对称）。"""
    b = "vine_relief"
    # 两侧浮雕纹框
    g.pair(b, "vine_border_t", (1.25, 0.90, -2.10), (1.45, 1.10, 2.10), "iron_lt")
    g.pair(b, "vine_border_b", (1.25, -0.90, -2.10), (1.45, -0.70, 2.10), "iron_lt")

    # S形缠枝主茎
    g.pair(b, "vine_stem_seg1", (1.28, 0.20, -1.90), (1.48, 0.70, -1.05), "iron_lt")
    g.pair(b, "vine_stem_seg2", (1.30, -0.60, -1.10), (1.50, 0.30, -0.15), "iron_lt")
    g.pair(b, "vine_stem_seg3", (1.28, 0.10, -0.20), (1.48, 0.70, 0.80), "iron_lt")
    g.pair(b, "vine_stem_seg4", (1.30, -0.60, 0.75), (1.50, 0.20, 1.80), "iron_lt")

    # 卷草叶瓣与花蕾
    g.pair(b, "vine_leaf_1", (1.32, 0.60, -1.60), (1.52, 0.90, -1.25), "iron_lt")
    g.pair(b, "vine_leaf_2", (1.32, -0.80, -0.80), (1.52, -0.40, -0.45), "iron_lt")
    g.pair(b, "vine_leaf_3", (1.32, 0.60, 0.20), (1.52, 0.90, 0.60), "iron_lt")
    g.pair(b, "vine_leaf_4", (1.32, -0.80, 1.10), (1.52, -0.40, 1.50), "iron_lt")
    g.pair(b, "vine_bud_1", (1.32, 0.25, -0.65), (1.52, 0.55, -0.35), "iron_lt")
    g.pair(b, "vine_bud_2", (1.32, -0.35, 0.35), (1.52, -0.05, 0.65), "iron_lt")

    # 阴刻地纹暗槽（错开 Z 消除共面）
    g.pair(b, "vine_shadow_pit1", (1.26, 0.10, -1.40), (1.42, 0.40, -0.92), "iron_dk")
    g.pair(b, "vine_shadow_pit2", (1.26, -0.30, 0.15), (1.42, -0.00, 0.70), "iron_dk")


def part_keyhole_plate(g: Gear) -> None:
    """锁尾钥匙插孔：后臀平直开槽钥匙孔与锁芯导向板（严格对称于 X=0）。"""
    b = "keyhole_plate"
    # 护孔铁面板
    g.box(b, "keyhole_escutcheon", (-0.95, -1.00, 3.25), (0.95, 0.60, 3.55), "iron")
    g.box(b, "keyhole_rim_t", (-0.85, 0.50, 3.32), (0.85, 0.72, 3.62), "iron_lt")
    g.box(b, "keyhole_rim_b", (-0.85, -1.10, 3.32), (0.85, -0.92, 3.62), "iron_lt")

    # 钥匙插入字孔（错开 Z 消除共面）
    g.box(b, "keyhole_slot_vert", (-0.15, -0.70, 3.42), (0.15, 0.40, 3.75), "void")
    g.box(b, "keyhole_slot_cross", (-0.35, -0.50, 3.35), (0.35, -0.10, 3.68), "void")

    # 磨光包浆导向斜面
    g.pair(b, "keyhole_guide_bevel", (0.15, -0.60, 3.42), (0.45, 0.30, 3.72), "iron_lt")
    g.pair(b, "keyhole_guide_lip", (0.35, -0.40, 3.45), (0.65, 0.10, 3.75), "iron_lt")
    g.pair(b, "keyhole_rivet", (0.65, -0.80, 3.48), (0.85, -0.50, 3.70), "iron_lt")


def part_bronze_spring(g: Gear) -> None:
    """外露铜簧片：左侧腹下缝隙露出的双叉青铜弹簧片（不对称）。"""
    b = "bronze_spring"
    g.box(b, "spring_anchor_block", (-1.42, -0.60, -0.50), (-1.15, -0.15, 0.50), "bronze")
    g.box(b, "spring_leaf_upper", (-1.52, -0.00, -0.40), (-1.22, 0.30, 0.40), "bronze_lt")
    g.box(b, "spring_leaf_lower", (-1.52, -0.50, -0.40), (-1.22, -0.15, 0.40), "bronze_lt")
    g.box(b, "spring_gap_void", (-1.55, -0.12, -0.30), (-1.20, -0.02, 0.30), "void")
    g.box(b, "spring_pin_f", (-1.48, -0.50, -0.62), (-1.18, -0.20, -0.35), "bronze")
    g.box(b, "spring_pin_b", (-1.48, -0.50, 0.35), (-1.18, -0.20, 0.62), "bronze")
    g.box(b, "spring_catch_tooth", (-1.45, 0.10, -0.12), (-1.15, 0.42, 0.12), "bronze_lt")


def part_rune_slot(g: Gear) -> None:
    """末法空置阵片位：右侧原本嵌灵气符骨的方形内凹暗槽（末法核心主题，不对称）。"""
    b = "rune_slot"
    g.box(b, "rune_cavity_void", (1.28, -0.30, 0.15), (1.58, 0.70, 1.50), "void")
    g.box(b, "rune_frame_t", (1.25, 0.72, 0.05), (1.48, 0.95, 1.60), "iron_dk")
    g.box(b, "rune_frame_b", (1.25, -0.50, 0.05), (1.48, -0.25, 1.60), "iron_dk")
    g.box(b, "rune_frame_f", (1.25, -0.40, -0.05), (1.48, 0.80, 0.20), "iron_dk")
    g.box(b, "rune_frame_r", (1.25, -0.40, 1.45), (1.48, 0.80, 1.70), "iron_dk")
    g.box(b, "rune_dead_trace1", (1.26, 0.15, 0.35), (1.38, 0.35, 1.30), "iron_dk")
    g.box(b, "rune_dead_trace2", (1.26, -0.10, 0.55), (1.38, 0.55, 0.75), "iron_dk")
    g.box(b, "rune_retaining_clip", (1.32, 0.55, 0.25), (1.52, 0.82, 0.55), "rust")
    g.box(b, "rune_corner_stud", (1.32, -0.42, 0.12), (1.52, -0.12, 0.42), "rust_dk")


def part_tie_cord(g: Gear) -> None:
    """拴钥细麻绳：从锁梁左侧系紧下垂连接钥匙的编绳（挂件独立骨）。"""
    b = "tie_cord"
    g.box(b, "cord_loop_shackle", (-0.68, 2.10, -1.85), (-0.12, 2.75, -1.20), "cord_dk")
    g.box(b, "cord_knot_main", (-0.98, 2.00, -1.75), (-0.42, 2.55, -1.10), "cord")
    g.box(b, "cord_strand_1", (-1.48, 1.70, -1.45), (-0.82, 2.25, -1.00), "cord")
    g.box(b, "cord_strand_2", (-1.98, 1.40, -1.15), (-1.32, 1.95, -0.70), "cord_dk")
    g.box(b, "cord_strand_3", (-2.38, 1.00, -0.85), (-1.72, 1.65, -0.40), "cord")
    g.box(b, "cord_strand_4", (-2.68, 0.60, -0.55), (-2.02, 1.25, -0.10), "cord_dk")
    g.box(b, "cord_knot_bow", (-2.88, 0.40, -0.35), (-2.22, 1.05, 0.25), "cord_dk")
    g.box(b, "cord_tail_fringe", (-2.78, 0.00, -0.25), (-2.32, 0.55, 0.15), "cord")
    g.box(b, "cord_bead_stopper", (-2.82, 0.80, -0.15), (-2.42, 1.12, 0.15), "bronze")
    g.box(b, "cord_fringe_tip", (-2.65, -0.35, -0.15), (-2.35, 0.05, 0.12), "cord_dk")


def part_key_bow(g: Gear) -> None:
    """钥匙方圈首：镂空如意方圈匙纽与打孔绳耳（挂件独立骨）。"""
    b = "key_bow"
    cx, cy, cz = -2.60, 0.85, 0.00
    # 镂空方圈
    g.box(b, "bow_ring_t", (cx - 0.22, cy + 0.50, cz - 0.80), (cx + 0.22, cy + 0.95, cz + 0.80), "iron")
    g.box(b, "bow_ring_b", (cx - 0.22, cy - 0.95, cz - 0.80), (cx + 0.22, cy - 0.50, cz + 0.80), "iron")
    g.box(b, "bow_ring_f", (cx - 0.22, cy - 0.60, cz - 0.85), (cx + 0.22, cy + 0.60, cz - 0.40), "iron_lt")
    g.box(b, "bow_ring_r", (cx - 0.22, cy - 0.60, cz + 0.40), (cx + 0.22, cy + 0.60, cz + 0.85), "iron")
    g.box(b, "bow_hollow_void", (cx - 0.25, cy - 0.50, cz - 0.40), (cx + 0.25, cy + 0.50, cz + 0.40), "void")

    # 如意折角起棱包浆
    g.box(b, "bow_corner_tf", (cx - 0.25, cy + 0.40, cz - 0.95), (cx + 0.25, cy + 0.85, cz - 0.50), "iron_lt")
    g.box(b, "bow_corner_tb", (cx - 0.25, cy + 0.40, cz + 0.50), (cx + 0.25, cy + 0.85, cz + 0.95), "iron_lt")
    g.box(b, "bow_corner_bf", (cx - 0.25, cy - 0.85, cz - 0.95), (cx + 0.25, cy - 0.40, cz - 0.50), "iron")
    g.box(b, "bow_corner_bb", (cx - 0.25, cy - 0.85, cz + 0.50), (cx + 0.25, cy - 0.40, cz + 0.95), "iron")

    # 匙圈颈部收束环与顶纽
    g.box(b, "bow_neck_collar", (cx - 0.25, cy - 1.25, cz - 0.30), (cx + 0.25, cy - 0.80, cz + 0.30), "iron_lt")
    g.box(b, "bow_bead_crest", (cx - 0.28, cy + 0.90, cz - 0.25), (cx + 0.28, cy + 1.20, cz + 0.25), "bronze_lt")
    g.box(b, "bow_ear_left", (cx - 0.25, cy - 0.25, cz - 1.05), (cx + 0.25, cy + 0.25, cz - 0.75), "iron_lt")
    g.box(b, "bow_ear_right", (cx - 0.25, cy - 0.25, cz + 0.75), (cx + 0.25, cy + 0.25, cz + 1.05), "iron_lt")


def part_key_stem(g: Gear) -> None:
    """细长钥匙杆：锻铁阶梯圆杆与定位环挡（挂件独立骨）。"""
    b = "key_stem"
    cx, cz = -2.60, 0.00
    g.box(b, "stem_shaft_upper", (cx - 0.18, -1.15, cz - 0.18), (cx + 0.18, -0.40, cz + 0.18), "iron")
    g.box(b, "stem_shaft_lower", (cx - 0.18, -1.85, cz - 0.18), (cx + 0.18, -1.10, cz + 0.18), "iron")
    g.box(b, "stem_stop_ring", (cx - 0.28, -1.35, cz - 0.28), (cx + 0.28, -1.00, cz + 0.28), "iron_lt")
    g.box(b, "stem_guide_groove", (cx - 0.15, -1.75, cz - 0.25), (cx + 0.15, -0.60, cz - 0.12), "iron_dk")
    g.box(b, "stem_rib_collar", (cx - 0.22, -0.70, cz - 0.22), (cx + 0.22, -0.45, cz + 0.22), "iron_lt")
    g.box(b, "stem_collar_notch", (cx - 0.25, -1.65, cz - 0.08), (cx + 0.25, -1.45, cz + 0.08), "iron_dk")


def part_key_bit(g: Gear) -> None:
    """三阶开簧齿板：端部精密开槽齿牙，用于压合铜簧（挂件独立骨）。"""
    b = "key_bit"
    cx, cz = -2.60, 0.00
    g.box(b, "bit_base_block", (cx - 0.22, -2.45, cz - 0.20), (cx + 0.22, -1.80, cz + 0.65), "iron_lt")
    g.box(b, "bit_tooth_1", (cx - 0.25, -2.85, cz + 0.30), (cx + 0.25, -2.20, cz + 0.75), "iron_lt")
    g.box(b, "bit_tooth_2", (cx - 0.25, -3.05, cz + 0.00), (cx + 0.25, -2.40, cz + 0.45), "iron_lt")
    g.box(b, "bit_tooth_3", (cx - 0.22, -3.25, cz - 0.25), (cx + 0.22, -2.60, cz + 0.15), "iron_lt")
    g.box(b, "bit_slot_gap", (cx - 0.28, -2.95, cz + 0.12), (cx + 0.28, -2.30, cz + 0.35), "void")
    g.box(b, "bit_lead_chamfer", (cx - 0.22, -3.35, cz - 0.30), (cx + 0.22, -3.00, cz - 0.02), "iron")
    g.box(b, "bit_catch_lip", (cx - 0.25, -2.60, cz + 0.62), (cx + 0.25, -2.30, cz + 0.85), "iron_lt")
    g.box(b, "bit_side_bevel", (cx - 0.28, -2.75, cz - 0.15), (cx + 0.28, -2.45, cz + 0.55), "iron_dk")


# ───────────────────────────────────────────────────────────── 门禁与共面自动错开
def deconflict_and_fix(g: Gear) -> None:
    """消除不对称部件内的 Z-fighting 共面，并确保无薄片。"""
    all_els = [e for e in g.rig.elements if not any(e.get("rotation", [0, 0, 0]))]
    asym_uuids = set()
    for b_name in ASYM_BONES:
        if b_name in g.rig.bones:
            asym_uuids.update(g.rig.bones[b_name]["children"])

    # 1. 修正尺寸 < 0.22 的薄片
    for e in all_els:
        for ax in range(3):
            s = e["to"][ax] - e["from"][ax]
            if s < 0.22:
                d = 0.22 - s
                e["from"][ax] = round(e["from"][ax] - d / 2, 4)
                e["to"][ax] = round(e["to"][ax] + d / 2, 4)

    # 2. 消除不对称件与其它件的共面冲突
    asym_els = [e for e in all_els if e["uuid"] in asym_uuids]
    for i, e in enumerate(asym_els):
        idx = all_els.index(e)
        for _ in range(25):
            moved = False
            for f in all_els[:idx]:
                for ax in range(3):
                    o1, o2 = [a for a in range(3) if a != ax]
                    if min(e["to"][o1], f["to"][o1]) - max(e["from"][o1], f["from"][o1]) <= 1e-4:
                        continue
                    if min(e["to"][o2], f["to"][o2]) - max(e["from"][o2], f["from"][o2]) <= 1e-4:
                        continue
                    for side, sign in (("from", -1), ("to", 1)):
                        if abs(e[side][ax] - f[side][ax]) < 1e-4:
                            e[side][ax] = round(e[side][ax] + sign * 0.003 * (1 + (i % 5)), 4)
                            moved = True
            if not moved:
                break


class TieSuoYueSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "lock_casing": part_lock_casing,
    "ox_head": part_ox_head,
    "ox_horns": part_ox_horns,
    "ox_legs": part_ox_legs,
    "ox_tail": part_ox_tail,
    "shackle_u": part_shackle_u,
    "vine_relief": part_vine_relief,
    "keyhole_plate": part_keyhole_plate,
    "bronze_spring": part_bronze_spring,
    "rune_slot": part_rune_slot,
    "tie_cord": part_tie_cord,
    "key_bow": part_key_bow,
    "key_stem": part_key_stem,
    "key_bit": part_key_bit,
}

SPEC = TieSuoYueSpec(
    gear_id="tie_suo_yue",
    model_name="TieSuoYue",
    title="铁锁与钥 / tie_suo_yue",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="手持小件 · 铸铁卧牛弹簧锁+牛头双角四短腿+锁梁穿牛背+独立细钥",
    worn_focus=((5.8, 13.0, -0.6), 18.0),
    tex_views=(
        ("3/4 正右斜特写", 152.0, 18.0),
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("RIGHT 右侧（+x）", -90.0, 0.0),
    ),
    texture_overrides={
        "iron": "metal_iron",
        "iron_lt": "metal_iron_lt",
        "iron_dk": "metal_vine_line",
        "rust": "metal_rust",
        "rust_dk": "metal_rust_dk",
        "bronze": "metal_bronze",
        "bronze_lt": "metal_bronze_lt",
        "cord": "metal_cord",
        "cord_dk": "metal_cord_dk",
    },
    category="metalware",
    scale_mode="held",
    held_offset=(5.8, 13.0, -0.6),
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
