#!/usr/bin/env python3
"""弯形兵器的 Round 1 审阅图流程：写 bbmodel → 六视图 / 比例对照 / 并排比例 / 持握姿势 / 骨骼树 → check.png。

各兵器的生成器（``gen_<id>.py``）把自己的常量与度量函数放在模块里，这里只负责拼图。
生成器模块需提供：

- ``WEAPON_ID`` ``MODEL_NAME`` ``OUT_DIR`` ``TOTAL_LEN`` ``GRIP_Y`` ``BONES`` ``REF_PNG`` ``REF_PANEL_W`` ``TITLE``
- ``POSE``：``"single_hand"`` / ``"polearm"`` / ``"bow"`` / ``"worn"``（挂件，另需 ``WORN_OFFSET`` ``WORN_TITLE``）
- ``PROP_YAW``：比例对照里渲染模型的视角（要让模型和参考图朝向一致）
- ``MEASURE_EXCLUDE_BONES``（可选）：量模型剪影前先去掉这些骨上的方块（弓弦）
- ``REVIEW_DIR``（可选）：审阅图目录，默认 img/weapons/<id>
- ``SKIP_IOU``（可选）：参考图是透视图时不画 IoU 行
- ``REF_BOX``（可选）：参考图是拼版时，主视图所在的 (x0, y0, x1, y1)
- ``REF_STRING_PX``（可选，默认 1）：参考图去弦用的横向开运算宽度（弓设 9）
- ``REF_SHADOW_PX``（可选，默认 22）：去地面阴影的竖向开运算长度，细斜杆件的弓要设 1
- ``measure(mask) -> dict[str, float]``：从剪影量出的各部件百分比；参考图和模型用同一个函数
- ``METRICS``：``[(标签, 度量键), ...]``，对照表的行

「参考图」一列由 ``measure`` 作用在参考图剪影上得到，「模型」一列作用在模型渲染剪影上，
不读设计意图，所以改了几何数字会直接反映在表里。
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import gear_kit  # noqa: E402
import weapon_review_kit  # noqa: E402
import weapon_shape_kit as kit  # noqa: E402


class _GearView:
    """给 gear_kit.bone_tree_panel 用的最小适配。"""

    def __init__(self, rig: Rig, bones: list) -> None:
        self.rig = rig
        self.bones = bones

    def total(self) -> int:
        return len(self.rig.elements)


def _ref_panel_values(mod, ref_mask: np.ndarray) -> dict[str, float]:
    return mod.measure(ref_mask)


def write_and_review(mod, rig: Rig) -> Path:
    mod.OUT_DIR.mkdir(parents=True, exist_ok=True)
    review_dir = getattr(mod, "REVIEW_DIR", kit.REVIEW_ROOT / mod.WEAPON_ID)
    review_dir.mkdir(parents=True, exist_ok=True)

    doc = rig.bbmodel(mod.MODEL_NAME)
    model_path = mod.OUT_DIR / f"{mod.MODEL_NAME}.bbmodel"
    model_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    count = len(rig.elements)

    six = gear_kit.render_six_views(model_path, f"{mod.TITLE} · {count} 方块 · 全长 {mod.TOTAL_LEN:.1f} px")

    if mod.POSE == "worn":     # 挂件：不是手持，改画「穿戴」栏
        pose_panel, _ = kit.worn_panel(doc, mod.WORN_OFFSET, mod.OUT_DIR, mod.TOTAL_LEN, mod.WORN_TITLE)
    else:
        pose_kwargs = {"pose_type": "bow"} if mod.POSE == "bow" else {"is_polearm": mod.POSE == "polearm"}
        pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
            doc, grip_y=mod.GRIP_Y, weapon_len=mod.TOTAL_LEN, out_dir=mod.OUT_DIR, **pose_kwargs)

    ref_mask = kit.ref_mask(mod.REF_PNG, mod.REF_PANEL_W, shadow_px=getattr(mod, "REF_SHADOW_PX", 22),
                            string_px=getattr(mod, "REF_STRING_PX", 1), box=getattr(mod, "REF_BOX", None))
    ref_crop = kit.crop_ref_view(mod.REF_PNG, ref_mask)
    model_img, _ = render(model_path, yaw=mod.PROP_YAW, pitch=0.0, size=900, bg=gear_kit.BG)
    exclude = getattr(mod, "MEASURE_EXCLUDE_BONES", ())
    if exclude:
        # 参考图剪影里弓弦已经去掉了，模型这边也去掉弦再量，两边口径一致
        drop = {uid for b in exclude for uid in rig.bones[b]["children"]}
        measure_doc = dict(doc, elements=[e for e in doc["elements"] if e["uuid"] not in drop])
        measure_path = mod.OUT_DIR / "_tmp_measure.bbmodel"
        measure_path.write_text(json.dumps(measure_doc), encoding="utf-8")
        measure_img, _ = render(measure_path, yaw=mod.PROP_YAW, pitch=0.0, size=900, bg=gear_kit.BG)
        measure_path.unlink()
        model_m = kit.model_mask(measure_img, gear_kit.BG)
    else:
        model_m = kit.model_mask(model_img, gear_kit.BG)

    ref_vals = mod.measure(ref_mask)
    mod_vals = mod.measure(model_m)
    metrics = [(label, ref_vals[key], mod_vals[key]) for label, key in mod.METRICS]
    iou = kit.silhouette_iou(ref_mask, model_m)
    if not getattr(mod, "SKIP_IOU", False):     # 参考图是透视图（杆是斜的）时，IoU 没有意义
        metrics.append(("剪影重合 IoU（同高缩放，100 = 完全重合）", 100.0, iou * 100.0))

    prop = weapon_review_kit.build_proportion_panel(
        ref_crop, model_img, metrics, [], target_h=six.height)

    tree = gear_kit.bone_tree_panel(
        _GearView(rig, mod.BONES), min_bones=6, height=six.height + pose_panel.height + 15)

    def scaled(path: Path, width: int = 640) -> Image.Image | None:
        if not path.exists():
            return None
        im = Image.open(path).convert("RGB")
        return im.resize((width, round(im.height * width / im.width)), Image.Resampling.LANCZOS)

    out = review_dir / "check.png"
    weapon_review_kit.assemble_check_image(
        six_views_img=six,
        scale_pose_panel=pose_panel,
        prop_panel=prop,
        tree_panel=tree,
        ref_scaled=scaled(mod.REF_PNG),
        exp_scaled=scaled(review_dir / f"{mod.WEAPON_ID}_exploded.png"),
        out_path=out,
        title=f"{mod.TITLE}（Round 1 几何平涂版；无贴图；剪影取自参考图）",
    )

    print("\n=== 比例对照（参考图 vs 模型渲染剪影，差超 10 个百分点要继续改）===")
    bad = 0
    for label, r, m in (metrics if getattr(mod, "SKIP_IOU", False) else metrics[:-1]):
        flag = "OK " if abs(r - m) <= 10 else "超标"
        bad += flag != "OK "
        print(f"  [{flag}] {label}: 参考 {r:.1f}% | 模型 {m:.1f}% | 差 {m - r:+.1f}")
    print(f"  剪影 IoU {iou * 100:.1f}%")
    return out
