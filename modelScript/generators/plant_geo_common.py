"""第一批贴地灵植的共享体素作者工具。

每个植物仍有自己的生成器和部件函数；这里仅放重复的 bbmodel 装配、方块空间平移、
贴图和 Round 2 门禁。坐标先在以方块中心为原点的作者空间里搭，写盘时再平移到 0..16。
"""

from __future__ import annotations

import base64
import copy
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from bbmodel_maker.gates.gatekit import gate_degenerate, gate_orphans, gate_overflow
from bbmodel_maker.rig.rigkit import Rig

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "modelScript" / "models"
OUT = ROOT / "modelScript" / "out"


def pad(
    rig: Rig,
    bone: str,
    name: str,
    center: tuple[float, float, float],
    size: tuple[float, float, float],
    mat: str,
    *,
    rotation: tuple[float, float, float] | None = None,
) -> None:
    """加一个以中心点定位的体素块。"""

    x, y, z = center
    w, h, d = size
    origin = (x - w / 2, y, z - d / 2)
    target = (x + w / 2, y + h, z + d / 2)
    rig.cube(bone, name, origin, target, mat=mat, rot=rotation, org=center)


def strand(
    rig: Rig,
    bone: str,
    name: str,
    start: tuple[float, float, float],
    end: tuple[float, float, float],
    radius: float,
    mat: str,
) -> None:
    """沿两点铺一段有方向的方柱，适合叶脉、根须和藻叶。"""

    rig.shaft(bone, name, start, end, radius, max(radius * 0.8, 0.12), mat=mat)


def curved_vine_chain(
    rig: Rig,
    bone: str,
    prefix: str,
    points: list[tuple[float, float, float]],
    radius_start: float,
    radius_end: float,
    mat: str,
) -> None:
    """沿 3D 曲线铺细藤连杆，端点精确贴合相邻节点。

    这里复用蜕骨藤生成器的旋转连杆算法：`Rig.shaft` 通过 `shaft_box` 计算每段
    的 pitch / yaw 与旋转轴，因此螺旋和垂挂曲线不会退化成一排竖直方块。
    """

    if len(points) < 2:
        raise ValueError(f"{prefix}: 曲线至少需要两个节点")
    last = len(points) - 2
    for index, (start, end) in enumerate(zip(points, points[1:])):
        progress = index / max(1, last)
        radius = radius_start + (radius_end - radius_start) * progress
        rig.shaft(
            bone,
            f"{prefix}_{index:02d}",
            start,
            end,
            radius,
            max(radius * 0.8, 0.12),
            mat=mat,
        )
def leaf(
    rig: Rig,
    bone: str,
    name: str,
    center: tuple[float, float, float],
    width: float,
    depth: float,
    height: float,
    mat: str,
    *,
    tilt: tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> None:
    """加一片有轻微倾角的厚叶，厚度留出体素像素。"""

    pad(rig, bone, name, center, (width, height, depth), mat, rotation=tilt)


def shift_to_block_space(model: dict) -> None:
    """把中心作者空间平移成客户端约定的 0..16 方块空间。"""

    def move(point: list[float] | None) -> None:
        if point is not None:
            point[0] += 8.0
            point[2] += 8.0

    for element in model["elements"]:
        move(element.get("from"))
        move(element.get("to"))
        move(element.get("origin"))

    def move_node(node: dict | str) -> None:
        if isinstance(node, str):
            return
        move(node.get("origin"))
        for child in node.get("children", []):
            move_node(child)

    for node in model.get("outliner", []):
        move_node(node)


def write_model(model_name: str, rig: Rig) -> Path:
    """写作者稿和可人工查看的 64×64 调色板贴图。"""

    MODELS.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    model = rig.bbmodel(model_name, namespace="bong")
    shift_to_block_space(model)
    path = MODELS / f"{model_name}.bbmodel"
    path.write_text(json.dumps(model, indent=2, ensure_ascii=False), encoding="utf-8")
    source = model["textures"][0]["source"].split(",", 1)[1]
    (OUT / f"{model_name}.png").write_bytes(base64.b64decode(source))
    return path


@dataclass(frozen=True)
class GateResult:
    key: str
    label: str
    violations: tuple[str, ...]

    @property
    def ok(self) -> bool:
        return not self.violations


class PlantGates:
    """资产级结构门禁，所有门都带一个能命中它的差分注入器。"""

    def __init__(self, title: str) -> None:
        self.title = title

    @staticmethod
    def _checks(rig: Rig) -> list[tuple[str, str, list[str]]]:
        orphan = gate_orphans(rig)
        overflow = gate_overflow(rig, shift=(8.0, 0.0, 8.0))
        degenerate = gate_degenerate(rig, min_thickness=0.2)
        lo, hi = rig.bounds()
        height = []
        if lo[1] < -0.01:
            height.append(f"最低点 {lo[1]:.2f} < 0")
        if hi[1] > 14.0:
            height.append(f"最高点 {hi[1]:.2f} > 14")
        return [
            ("orphans", "孤儿 element", orphan),
            ("overflow", "越出 0..16 方块空间", overflow),
            ("degenerate", "退化薄片 (<0.2px)", degenerate),
            ("height", "生长高度与地面", height),
        ]

    def report(self, rig: Rig, **_kwargs: object) -> int:
        lo, hi = rig.bounds()
        print(f"{self.title} 自检:")
        print(
            f"  bbox: {hi[0] - lo[0]:.2f}×{hi[1] - lo[1]:.2f}×{hi[2] - lo[2]:.2f}px; "
            f"cubes={len(rig.elements)} bones={len(rig.bones)}"
        )
        total = 0
        for _key, label, violations in self._checks(rig):
            mark = "✓" if not violations else "✗"
            print(f"  {mark} {label}: {len(violations)}")
            total += len(violations)
        print(f"  → 共 {total} 处违例")
        return total

    @staticmethod
    def _with_element(rig: Rig, index: int, mutate: Callable[[dict], None]) -> Rig:
        probe = copy.deepcopy(rig)
        mutate(probe.elements[index])
        return probe

    @staticmethod
    def _inject_orphan(rig: Rig) -> Rig:
        probe = copy.deepcopy(rig)
        element = probe.elements[-1]
        for bone in probe.bones.values():
            if element["uuid"] in bone["children"]:
                bone["children"].remove(element["uuid"])
                break
        return probe

    def self_test(self, rig: Rig, **_kwargs: object) -> int:
        """验证每道门在干净模型为零、注入对应缺陷后会报错。"""

        injectors: list[Callable[[Rig], Rig]] = [
            self._inject_orphan,
            lambda r: self._with_element(
                r, 0, lambda e: (e["from"].__setitem__(0, e["from"][0] + 20),
                                  e["to"].__setitem__(0, e["to"][0] + 20))),
            lambda r: self._with_element(
                r, 0, lambda e: e["to"].__setitem__(1, e["from"][1] + 0.05)),
            lambda r: self._with_element(
                r, 0, lambda e: (e["from"].__setitem__(1, e["from"][1] + 20),
                                  e["to"].__setitem__(1, e["to"][1] + 20))),
        ]
        broken = 0
        print(f"{self.title} 差分自证:")
        for (key, label, clean), inject in zip(self._checks(rig), injectors):
            bad_clean = bool(clean)
            injected = self._checks(inject(rig))[{"orphans": 0, "overflow": 1, "degenerate": 2, "height": 3}[key]][2]
            if bad_clean or not injected:
                broken += 1
                print(f"  ✗ {label}: clean={len(clean)}, injected={len(injected)}")
            else:
                print(f"  ✓ {label}: clean=0 → injected={len(injected)}")
        print(f"  → {len(injectors) - broken}/{len(injectors)} 道门有鉴别力")
        return broken

    def run_all(self, rig: Rig) -> list[GateResult]:
        return [GateResult(key, label, tuple(violations))
                for key, label, violations in self._checks(rig)]


def build_rig(mats: dict[str, tuple[int, int, int]], parts: tuple[Callable[[Rig], None], ...]) -> Rig:
    rig = Rig(mats, tex=64, swatch=8)
    for part in parts:
        part(rig)
    return rig
