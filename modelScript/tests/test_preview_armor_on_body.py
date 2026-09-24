from __future__ import annotations

import sys
import unittest
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in ("generators", "exporters", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_hide_armor
from bbmodel_maker.workbench import preview_armor_on_body as P
from bbmodel_maker.model.armor_model_common import MOUNT_PIVOT

BODY_CENTER = (0.0, 24.0, 0.0)


def _blank() -> Image.Image:
    return Image.new("RGBA", (64, 64))


def _build(part):
    return P.build_player_bbmodel(part, _blank(), _blank())


def _armor_root(model: dict) -> dict:
    # outliner[0] 是甲件组，[1] 是玩家组
    return model["outliner"][0]


def _mounts_of(part) -> set[str]:
    return {c.mount for c in part.cubes}


def _parts_by_span():
    single, multi = [], []
    for part in gen_hide_armor.parts():
        (single if len(_mounts_of(part)) == 1 else multi).append(part)
    return single, multi


class OutlinerPivotTest(unittest.TestCase):
    """回归锁：跨挂点甲件（leggings / boots）的右侧曾套用左侧枢轴。

    根因是分组只按装配名（band / splint / …）分，而这些装配每个都同时含左右
    两侧的 cube，组心却一律取 part.cubes[0].mount —— 在 Blockbench 里拖旋转
    时右腿/右脚会绕着左侧枢轴甩出去，预览摆姿势就看不出真实穿帮。
    """

    def test_fixture_still_has_both_single_and_multi_mount_parts(self) -> None:
        single, multi = _parts_by_span()
        self.assertTrue(single, "期望至少一件单挂点甲（helmet/chestplate），否则本测试覆盖不到该分支")
        self.assertTrue(multi, "期望至少一件跨挂点甲（leggings/boots），否则回归锁形同虚设")

    def test_multi_mount_parts_group_by_mount_with_own_pivot(self) -> None:
        _, multi = _parts_by_span()
        for part in multi:
            with self.subTest(part=part.key):
                root = _armor_root(_build(part))
                by_name = {g["name"]: g for g in root["children"]}
                self.assertEqual(
                    set(by_name), _mounts_of(part),
                    f"{part.key} 跨挂点时第一级必须是挂点组，实际是 {sorted(by_name)}",
                )
                for mount, group in by_name.items():
                    expected = MOUNT_PIVOT[mount]
                    self.assertEqual(
                        tuple(group["origin"]), tuple(expected),
                        f"{part.key}/{mount} 组心应为该挂点枢轴 {expected}，实际 {group['origin']}",
                    )

    def test_every_assembly_pivot_matches_its_own_mount(self) -> None:
        # 核心不变式：装配组的枢轴必须来自它自己的挂点，不能继承兄弟挂点的
        for part in gen_hide_armor.parts():
            root = _armor_root(_build(part))
            mounts = _mounts_of(part)
            with self.subTest(part=part.key):
                if len(mounts) == 1:
                    mount = next(iter(mounts))
                    for asm in root["children"]:
                        self.assertEqual(
                            tuple(asm["origin"]), tuple(MOUNT_PIVOT[mount]),
                            f"{part.key}/{asm['name']} 应用 {mount} 的枢轴",
                        )
                    continue
                for mount_group in root["children"]:
                    mount = mount_group["name"]
                    for asm in mount_group["children"]:
                        self.assertEqual(
                            tuple(asm["origin"]), tuple(MOUNT_PIVOT[mount]),
                            f"{part.key}/{mount}/{asm['name']} 应用 {mount} 的枢轴 "
                            f"{MOUNT_PIVOT[mount]}，实际 {asm['origin']}",
                        )

    def test_left_and_right_pivots_actually_differ(self) -> None:
        # 光对拍 MOUNT_PIVOT 还不够：若左右枢轴表本身相同，上面的断言会假绿
        _, multi = _parts_by_span()
        for part in multi:
            with self.subTest(part=part.key):
                xs = {MOUNT_PIVOT[m][0] for m in _mounts_of(part)}
                self.assertGreater(
                    len(xs), 1,
                    f"{part.key} 的左右挂点 x 枢轴相同，回归锁无法区分左右",
                )

    def test_single_mount_parts_keep_flat_assembly_layer(self) -> None:
        # 单挂点不该被"顺手"多套一层挂点组，否则 Blockbench 里的展开状态白洗一次
        single, _ = _parts_by_span()
        for part in single:
            with self.subTest(part=part.key):
                root = _armor_root(_build(part))
                names = {g["name"] for g in root["children"]}
                self.assertNotIn(
                    next(iter(_mounts_of(part))), names,
                    f"{part.key} 是单挂点，第一级应直接是装配名，实际出现挂点组 {names}",
                )
                expected = {c.name.split("_", 1)[0] for c in part.cubes}
                self.assertEqual(names, expected, f"{part.key} 装配分组与 cube 名首段不符")

    def test_root_pivot_never_claims_one_side_of_a_mixed_part(self) -> None:
        for part in gen_hide_armor.parts():
            root = _armor_root(_build(part))
            mounts = _mounts_of(part)
            with self.subTest(part=part.key):
                if len(mounts) == 1:
                    self.assertEqual(tuple(root["origin"]),
                                     tuple(MOUNT_PIVOT[next(iter(mounts))]))
                else:
                    self.assertEqual(
                        tuple(root["origin"]), BODY_CENTER,
                        f"{part.key} 混挂点，甲件根组心应取身体中心而不是任一侧",
                    )

    def test_grouping_covers_every_cube_exactly_once(self) -> None:
        # 分层重构最容易掉 cube：uuid 必须不重不漏地铺满 elements
        for part in gen_hide_armor.parts():
            model = _build(part)
            root = _armor_root(model)
            seen: list[str] = []

            def walk(node):
                for child in node["children"]:
                    if isinstance(child, dict):
                        walk(child)
                    else:
                        seen.append(child)

            walk(root)
            armor_uuids = [e["uuid"] for e in model["elements"]
                           if not e["name"].startswith("player_")]
            with self.subTest(part=part.key):
                self.assertEqual(len(seen), len(set(seen)), f"{part.key} 有 cube 被挂进多个组")
                self.assertEqual(sorted(seen), sorted(armor_uuids),
                                 f"{part.key} 分组后的 cube 集合与 elements 不一致")


if __name__ == "__main__":
    unittest.main()
