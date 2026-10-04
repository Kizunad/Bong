"""立方体共面冲突 (Z-fighting) 通用门禁与差分注入。

在体素/Blockbench 建模中，如果两个立方体在某一个轴上具有同向外表面（同为正向面或同为负向面），
且该外表面在同一平面上（坐标差在容差 tol 内），同时另两个轴的投影存在正面积重叠（> tol），
则在光栅化渲染时两表面深度一致，会产生高频噪点闪烁（即 Z-fighting）。

历史生成器里的 `_assert_no_coplanar_faces` 误将共面检查包裹在三轴正体积重叠条件
（`overlap_x > tol and overlap_y > tol and overlap_z > tol`）中，导致两个立方体
仅在单表面共面但整体无体积穿透时（例如一个较薄的表面外嵌贴片正好落在厚底座的外表面上），
三轴重叠不成立而直接漏判。

本模块提供统一、可复用的共面判定算子 `check_coplanar_faces` / `assert_no_coplanar_faces`，
并配套符合视觉资产纪律的差分注入器 `inject_single_face_coplanar`、`inject_collinear_non_overlapping`
与门禁自测试验 `self_test_coplanar_gate`。
"""

from __future__ import annotations

from typing import Any, Sequence


class CoplanarConflictError(ValueError, AssertionError):
    """当立方体之间存在共面面冲突 (Z-fighting) 时抛出的异常。

    同时继承 ValueError 和 AssertionError，以兼容各生成器与测试套件的历史捕获逻辑。
    """


def _cube_bounds(c: Any) -> tuple[str, list[float], list[float]]:
    """统一解析多种立方体格式，提取立方体名称、坐标下界 (min) 与上界 (max)。

    支持格式：
      - 元组格式 1: (bone, mat, name, [fx, fy, fz], [tx, ty, tz], ...)
      - 元组格式 2: (name, [fx, fy, fz], [tx, ty, tz])
      - 字典格式: {"name": str, "from": [...], "to": [...]}
      - 对象格式: 具有 name、origin、size 属性的类 (如 ArmorCube)
    """
    if isinstance(c, tuple):
        if len(c) >= 5 and isinstance(c[3], (list, tuple)) and isinstance(c[4], (list, tuple)):
            name = str(c[2])
            f, t = c[3], c[4]
            return (
                name,
                [min(float(f[i]), float(t[i])) for i in range(3)],
                [max(float(f[i]), float(t[i])) for i in range(3)],
            )
        if len(c) >= 3 and isinstance(c[1], (list, tuple)) and isinstance(c[2], (list, tuple)):
            name = str(c[0])
            f, t = c[1], c[2]
            return (
                name,
                [min(float(f[i]), float(t[i])) for i in range(3)],
                [max(float(f[i]), float(t[i])) for i in range(3)],
            )
        raise ValueError(f"无法识别的元组格式立方体: {c}")

    if isinstance(c, dict):
        name = str(c.get("name", "unnamed"))
        f = c["from"]
        t = c["to"]
        return (
            name,
            [min(float(f[i]), float(t[i])) for i in range(3)],
            [max(float(f[i]), float(t[i])) for i in range(3)],
        )

    if hasattr(c, "name") and hasattr(c, "origin") and hasattr(c, "size"):
        name = str(c.name)
        lo = [float(v) for v in c.origin]
        hi = [lo[i] + float(c.size[i]) for i in range(3)]
        return (
            name,
            [min(lo[i], hi[i]) for i in range(3)],
            [max(lo[i], hi[i]) for i in range(3)],
        )

    raise TypeError(f"不支持的立方体数据结构: {type(c)}")


def check_coplanar_faces(cubes: Sequence[Any], tol: float = 1e-4) -> list[str]:
    """检查立方体集是否存在共面面冲突 (Z-fighting)。

    判据：
      两个立方体在某一轴上的面坐标差在容差 tol 内，且另外两个轴的投影存在正面积重叠 (> tol) 时，
      判定为共面冲突。

    返回所有违规描述字符串列表。格式形如：
      "共面冲突: cube_a 与 cube_b 在 -X 面共面 (5.0000), 重叠区域 (1.200x0.800)"
    """
    violations: list[str] = []
    n = len(cubes)

    for i in range(n):
        n1, b1_min, b1_max = _cube_bounds(cubes[i])
        for j in range(i + 1, n):
            n2, b2_min, b2_max = _cube_bounds(cubes[j])

            for axis in range(3):
                ax_name = "XYZ"[axis]
                other_axes = [a for a in range(3) if a != axis]
                ov0 = min(b1_max[other_axes[0]], b2_max[other_axes[0]]) - max(
                    b1_min[other_axes[0]], b2_min[other_axes[0]]
                )
                ov1 = min(b1_max[other_axes[1]], b2_max[other_axes[1]]) - max(
                    b1_min[other_axes[1]], b2_min[other_axes[1]]
                )

                # 投影无正面积重叠（仅共线、共点或完全分离），不产生可见面闪烁
                if ov0 <= tol or ov1 <= tol:
                    continue

                # 负方向面同面共面冲突
                if abs(b1_min[axis] - b2_min[axis]) < tol:
                    violations.append(
                        f"共面冲突: {n1} 与 {n2} 在 -{ax_name} 面共面 ({b1_min[axis]:.4f}), 重叠区域 ({ov0:.3f}x{ov1:.3f})"
                    )

                # 正方向面同面共面冲突
                if abs(b1_max[axis] - b2_max[axis]) < tol:
                    violations.append(
                        f"共面冲突: {n1} 与 {n2} 在 +{ax_name} 面共面 ({b1_max[axis]:.4f}), 重叠区域 ({ov0:.3f}x{ov1:.3f})"
                    )

    return violations


def assert_no_coplanar_faces(cubes: Sequence[Any], tol: float = 1e-4) -> None:
    """断言立方体集无共面冲突；若存在冲突，抛出包含首条违规信息的 CoplanarConflictError。"""
    violations = check_coplanar_faces(cubes, tol=tol)
    if violations:
        raise CoplanarConflictError(violations[0])


def inject_single_face_coplanar(
    base: dict[str, Any] | None = None,
    axis: int = 2,
    is_max: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """构造一对『仅共享一个面』且另两轴有正投影面积重叠的立方体对。

    用于差分测试：验证共面判据能否准确捕获无正体积重叠、仅在单一表面重叠的情况。
    """
    if base is None:
        c1 = {"name": "base_cube", "from": [0.0, 0.0, 0.0], "to": [4.0, 4.0, 4.0]}
    else:
        c1 = dict(base)

    _, lo, hi = _cube_bounds(c1)
    other = [a for a in range(3) if a != axis]

    c2_from = [0.0, 0.0, 0.0]
    c2_to = [0.0, 0.0, 0.0]

    # 在另两轴上缩小范围，确保投影正交重叠且在边界内
    for a in other:
        span = hi[a] - lo[a]
        c2_from[a] = lo[a] + span * 0.25
        c2_to[a] = lo[a] + span * 0.75

    if is_max:
        # 共享最大面 (+面)
        c2_from[axis] = hi[axis] - 1.0
        c2_to[axis] = hi[axis]
    else:
        # 共享最小面 (-面)
        c2_from[axis] = lo[axis]
        c2_to[axis] = lo[axis] + 1.0

    c2 = {"name": "injected_single_face_coplanar", "from": c2_from, "to": c2_to}
    return c1, c2


def inject_collinear_non_overlapping(
    base: dict[str, Any] | None = None,
    shared_axis: int = 2,
    collinear_axis: int = 0,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """构造一对『同轴面坐标相同但另两轴投影不重叠』（仅共线、不重叠）的立方体对。

    用于差分测试：验证共面判据不会对边缘相碰或仅共线的立方体产生误报。
    """
    if base is None:
        c1 = {"name": "base_cube", "from": [0.0, 0.0, 0.0], "to": [2.0, 2.0, 2.0]}
    else:
        c1 = dict(base)

    _, lo, hi = _cube_bounds(c1)
    c2_from = list(lo)
    c2_to = list(hi)

    # 在 collinear_axis 上将 c2 挪到 c1 旁边紧贴边缘（接触面重叠为 0）
    c2_from[collinear_axis] = hi[collinear_axis]
    c2_to[collinear_axis] = hi[collinear_axis] + (hi[collinear_axis] - lo[collinear_axis])

    # 在 shared_axis 上保持 face 相同
    c2_from[shared_axis] = lo[shared_axis]
    c2_to[shared_axis] = hi[shared_axis]

    c2 = {"name": "injected_collinear", "from": c2_from, "to": c2_to}
    return c1, c2


def self_test_coplanar_gate() -> None:
    """运行共面门禁自身的差分自测试验。

    验证：
      1. 注入『只共享一个面』的立方体对 → 必须报出违规；
      2. 注入『同轴面坐标相同但另两轴投影不重叠』的立方体对 → 不得误报；
      3. 正常分离的立方体对 → 0 冲突。
    """
    # 1. 注入只共享一个面 → 必须拦截
    c1, c2 = inject_single_face_coplanar(axis=2, is_max=False)
    violations = check_coplanar_faces([c1, c2])
    if not violations:
        raise AssertionError("门禁自测试验失效：未能捕获只共享一个面的共面立方体缺陷！")
    if "-Z" not in violations[0]:
        raise AssertionError(f"门禁自测试验失效：报错面朝向未匹配期望 -Z 面: {violations[0]}")

    c1_max, c2_max = inject_single_face_coplanar(axis=0, is_max=True)
    violations_max = check_coplanar_faces([c1_max, c2_max])
    if not violations_max or "+X" not in violations_max[0]:
        raise AssertionError(f"门禁自测试验失效：未能捕获 +X 面单面共面缺陷: {violations_max}")

    # 2. 注入同轴面坐标相同但另两轴不重叠 (边缘接触) → 不得误报
    c1_col, c2_col = inject_collinear_non_overlapping(shared_axis=2, collinear_axis=0)
    collinear_violations = check_coplanar_faces([c1_col, c2_col])
    if collinear_violations:
        raise AssertionError(
            f"门禁自测试验失效：对边缘接触/仅共线立方体产生了误报: {collinear_violations}"
        )

    # 3. 正常分离立方体 → 0 冲突
    c_norm_a = {"name": "norm_a", "from": [0.0, 0.0, 0.0], "to": [1.0, 1.0, 1.0]}
    c_norm_b = {"name": "norm_b", "from": [2.0, 2.0, 2.0], "to": [3.0, 3.0, 3.0]}
    norm_violations = check_coplanar_faces([c_norm_a, c_norm_b])
    if norm_violations:
        raise AssertionError(f"门禁自测试验失效：正常分离模型误报冲突: {norm_violations}")
