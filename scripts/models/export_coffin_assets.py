#!/usr/bin/env python3
"""导出 4 档延寿棺 bbmodel → GeckoLib geo.json + 内嵌贴图 PNG + idle animation.json。

以 ``local_models/{Mundane,Jade,Stone,Bronze}Coffin.bbmodel`` 为基准（用户已在 Blockbench
精修），直接抽内嵌贴图存 PNG、把几何转成 geckolib ``geo.json``，免手工 Blockbench 导出。

转换关系（无坐标翻转，已用 CoffinCommon oracle 验证）：
  - geckolib cube ``origin`` = bbmodel element ``from``
  - ``size``                 = ``to - from``
  - per-face ``uv``          = ``[u1, v1]``          （bbmodel face uv = [u1,v1,u2,v2]）
  - per-face ``uv_size``     = ``[u2-u1, v2-v1]``    （保留负值=镜像翻转）
  - element ``rotation`` →   cube ``rotation`` + ``pivot`` = element ``origin``
  - ``texture_width/height`` = 实际导出 PNG 尺寸（GeckoLib 硬约束：必须相等，不自动缩放）

骨骼：每个 bbmodel 顶层 group → 一根 bone（parent=root）。group 有名则取名（小写）；
无名（生成器产出的 fmt5.0 group）按成员 cube 平均 Y 排序赋 base/body/lid（lid=最高）。
idle 动画对 ``lid`` 骨骼做轻微 Z 摆（参 ``coffin_common.animation.json`` 家族风格）。

用法:
  python3 scripts/models/export_coffin_assets.py            # 导出全部 4 档
  python3 scripts/models/export_coffin_assets.py --verify   # 仅跑 CoffinCommon oracle 自检
  python3 scripts/models/export_coffin_assets.py --tier jade
"""

from __future__ import annotations

import argparse
import base64
import io
import json
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[2]
MODELS = REPO / "local_models"
ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"
GEO_DIR = ASSETS / "geo"
TEX_DIR = ASSETS / "textures" / "entity"
ANIM_DIR = ASSETS / "animations"

# tier -> bbmodel 文件名 stem
TIERS = {
    "mundane": "MundaneCoffin",
    "jade": "JadeCoffin",
    "stone": "StoneCoffin",
    "bronze": "BronzeCoffin",
}

CUBE_FACES = ("north", "south", "east", "west", "up", "down")


# ---------------------------------------------------------------------------
# bbmodel 解析
# ---------------------------------------------------------------------------
def _load(path: Path) -> dict:
    return json.loads(path.read_text())


def _elements_by_uuid(bb: dict) -> dict:
    """uuid -> element（仅 cube；mesh 不支持，遇到报错）。"""
    out = {}
    for e in bb["elements"]:
        if e.get("type", "cube") != "cube":
            raise ValueError(f"不支持 mesh 元素 {e.get('name')!r}；棺材模型应为纯 cube")
        out[e["uuid"]] = e
    return out


def _cube_from_element(e: dict) -> dict:
    """bbmodel element → geckolib cube（origin/size/uv，必要时 rotation+pivot）。"""
    f, t = e["from"], e["to"]
    cube: dict = {
        "origin": [f[0], f[1], f[2]],
        "size": [t[0] - f[0], t[1] - f[1], t[2] - f[2]],
    }
    if e.get("inflate"):
        cube["inflate"] = e["inflate"]
    rot = e.get("rotation")
    if rot and any(rot):
        cube["rotation"] = list(rot)
        cube["pivot"] = list(e.get("origin", [0, 0, 0]))

    uv: dict = {}
    for fname, fd in e.get("faces", {}).items():
        if fname not in CUBE_FACES:
            continue
        u = fd.get("uv")
        if u is None:
            continue
        u1, v1, u2, v2 = u
        if fd.get("rotation"):
            raise ValueError(
                f"face {fname} 带 texture rotation={fd['rotation']}（element {e.get('name')!r}）"
                "；geckolib per-face uv 不支持，需在 Blockbench 烘平"
            )
        uv[fname] = {"uv": [u1, v1], "uv_size": [u2 - u1, v2 - v1]}
    cube["uv"] = uv
    return cube


def _group_cubes(group: dict, by_uuid: dict) -> list[dict]:
    """递归收集 group 下所有 element uuid 对应的 cube。

    嵌套子 group 若带 rotation/非零 origin（功能性变换），本函数的展平会把该变换静默丢掉、
    导出几何即错且不报错 —— 故显式拒绝（本导出器只对顶层 group 建 bone/施变换）。宁可撞红，
    也不静默产出错误几何。本批 4 档棺均为扁平结构，不触发；守卫防的是未来嵌套模型。
    """
    cubes = []
    for child in group.get("children", []):
        if isinstance(child, str):
            if child in by_uuid:
                cubes.append(_cube_from_element(by_uuid[child]))
        elif isinstance(child, dict):
            rot, org = child.get("rotation"), child.get("origin")
            if (rot and any(rot)) or (org and any(org)):
                raise ValueError(
                    f"嵌套 group {child.get('name')!r} 带变换 rotation={rot}/origin={org}；"
                    "导出器只支持顶层 group 变换，请在 Blockbench 展平或提升为顶层 group"
                )
            cubes.extend(_group_cubes(child, by_uuid))
    return cubes


def _mean_y(cubes: list[dict]) -> float:
    if not cubes:
        return 0.0
    return sum(c["origin"][1] + c["size"][1] / 2 for c in cubes) / len(cubes)


def _bbox(cubes: list[dict]) -> tuple[list[float], list[float]]:
    mn = [1e9, 1e9, 1e9]
    mx = [-1e9, -1e9, -1e9]
    for c in cubes:
        o, s = c["origin"], c["size"]
        for i in range(3):
            mn[i] = min(mn[i], o[i], o[i] + s[i])
            mx[i] = max(mx[i], o[i], o[i] + s[i])
    return mn, mx


def _assign_bone_names(groups_cubes: list[tuple[dict, list[dict]]]) -> list[str]:
    """3 个顶层 group → base/body/lid（按平均 Y 排序，lid=最高）。

    若 group 自带名字（如 Stone 的 base/body/lid）且恰为这三个，直接沿用其顺序映射；
    否则一律按 Y 排名赋名，保证 ``lid`` 永远是最高那组（供 idle 动画定位）。
    """
    n = len(groups_cubes)
    if n == 3:
        order = sorted(range(3), key=lambda i: _mean_y(groups_cubes[i][1]))
        names = [""] * 3
        names[order[0]] = "base"
        names[order[1]] = "body"
        names[order[2]] = "lid"
        return names
    # 非典型结构：用 group 名或 group_i，最高 Y 那组强制叫 lid
    names = []
    top = max(range(n), key=lambda i: _mean_y(groups_cubes[i][1])) if n else -1
    for i, (g, _) in enumerate(groups_cubes):
        if i == top:
            names.append("lid")
        elif g.get("name"):
            names.append(str(g["name"]).lower())
        else:
            names.append(f"group_{i}")
    return names


def build_geo(bb: dict, identifier: str) -> dict:
    by_uuid = _elements_by_uuid(bb)
    outliner = bb.get("outliner") or []
    top_groups = [g for g in outliner if isinstance(g, dict)]

    groups_cubes = [(g, _group_cubes(g, by_uuid)) for g in top_groups]
    bone_names = _assign_bone_names(groups_cubes)

    bones = [{"name": "root", "pivot": [0, 0, 0]}]
    all_cubes: list[dict] = []
    for (g, cubes), name in zip(groups_cubes, bone_names):
        all_cubes.extend(cubes)
        rot = g.get("rotation")
        if rot and any(rot) and g.get("origin"):
            # 带旋转的 group：origin 是功能性铰链，必须沿用
            pivot = list(g["origin"])
        elif cubes:
            # 静态 group：pivot 仅在 idle 对该骨骼施加旋转时充当铰链，统一取 bbox 中心(X/Z)+底面(Y)。
            # 不信任 group origin —— 生成器可能写入非居中脏值（实例：Stone 的 lid group origin X=-5.0），
            # 会让 idle 的 lid Z 摆绕左边缘偏摆、与其余档不一致（对峙 major 修复，2026-06-04）。
            mn, mx = _bbox(cubes)
            pivot = [(mn[0] + mx[0]) / 2, mn[1], (mn[2] + mx[2]) / 2]
        else:
            pivot = [0, 0, 0]
        bone: dict = {"name": name, "parent": "root", "pivot": pivot}
        if g.get("rotation") and any(g["rotation"]):
            bone["rotation"] = list(g["rotation"])
        bone["cubes"] = cubes
        bones.append(bone)

    # 任何没挂进 group 的散 element（理论上不该有）兜底进 root，避免漏渲染
    used_uuids: set[str] = set()

    def _collect_uuids(group):
        for ch in group.get("children", []):
            if isinstance(ch, str):
                used_uuids.add(ch)
            elif isinstance(ch, dict):
                _collect_uuids(ch)

    for g in top_groups:
        _collect_uuids(g)
    orphans = [
        _cube_from_element(e) for uid, e in by_uuid.items() if uid not in used_uuids
    ]
    if orphans:
        all_cubes.extend(orphans)
        bones.append({"name": "loose", "parent": "root", "pivot": [0, 0, 0], "cubes": orphans})

    # 后验：lid 骨骼（idle 唯一动画对象）pivot 的 X 必须等于其 cube X 中心，
    # 否则棺盖会绕偏轴侧摆 —— 把这类 bbmodel 脏 origin 在导出时撞红（对峙 major 守卫）。
    for b in bones:
        if b["name"] == "lid" and b.get("cubes"):
            lmn, lmx = _bbox(b["cubes"])
            cx = (lmn[0] + lmx[0]) / 2
            if abs(b["pivot"][0] - cx) > 1e-6:
                raise ValueError(
                    f"lid pivot X={b['pivot'][0]} != lid cube 中心 X={cx}；棺盖会绕偏轴摆动，检查 group origin"
                )

    mn, mx = _bbox(all_cubes)
    span_x, span_y, span_z = (mx[i] - mn[i] for i in range(3))
    return {
        "format_version": "1.12.0",
        "minecraft:geometry": [
            {
                "description": {
                    "identifier": identifier,
                    "texture_width": int(bb.get("resolution", {}).get("width", 64)),
                    "texture_height": int(bb.get("resolution", {}).get("height", 64)),
                    "visible_bounds_width": round(max(span_x, span_z) / 16 + 1, 2),
                    "visible_bounds_height": round(span_y / 16 + 0.5, 2),
                    "visible_bounds_offset": [
                        0,
                        round((mn[1] + mx[1]) / 2 / 16, 3),
                        0,
                    ],
                },
                "bones": bones,
            }
        ],
    }


def extract_texture(bb: dict) -> tuple[bytes, int, int]:
    src = bb["textures"][0]["source"]
    if src.startswith("data:"):
        src = src.split(",", 1)[1]
    raw = base64.b64decode(src)
    im = Image.open(io.BytesIO(raw)).convert("RGBA")
    # 重新编码为标准 PNG，确保字节稳定 + 尺寸权威
    buf = io.BytesIO()
    im.save(buf, format="PNG")
    return buf.getvalue(), im.width, im.height


# 档位递增的 idle 摆幅/时长 —— 沿用既有 coffin 家族「越高级摆动越显」叙事
# （coffin_common Z=0.5 / rare 1.0 / precious 1.5+len3.0）。延寿棺四档自成一档梯度：
# 凡木最克制、青铜最舒展，让 idle 也能读出档位差（对峙 minor 打磨，2026-06-04）。
IDLE_PROFILE = {
    "mundane": (0.4, 2.4),
    "jade": (0.6, 2.6),
    "stone": (0.8, 2.8),
    "bronze": (1.0, 3.0),
}


def build_animation(tier: str) -> dict:
    z_max, length = IDLE_PROFILE[tier]
    mid = round(length / 2, 3)
    return {
        "format_version": "1.8.0",
        "animations": {
            f"animation.bong.{tier}_coffin.idle": {
                "loop": True,
                "animation_length": length,
                "bones": {
                    "lid": {
                        "rotation": {
                            "0.0": [0, 0, 0],
                            str(mid): [0, 0, z_max],
                            str(length): [0, 0, 0],
                        }
                    }
                },
            }
        },
    }


# ---------------------------------------------------------------------------
# 校验 oracle：CoffinCommon.bbmodel cube 转换须复现 coffin_common.geo.json
# ---------------------------------------------------------------------------
def verify_oracle() -> bool:
    bb = _load(MODELS / "CoffinCommon.bbmodel")
    ref = _load(GEO_DIR / "coffin_common.geo.json")
    ref_cubes = {}
    for bone in ref["minecraft:geometry"][0]["bones"]:
        for c in bone.get("cubes", []):
            ref_cubes[tuple(c["origin"])] = c

    mine = [_cube_from_element(e) for e in bb["elements"]]
    ok = True
    for c in mine:
        key = tuple(c["origin"])
        r = ref_cubes.get(key)
        if r is None:
            print(f"  ✗ origin {key} 在参考 geo 中找不到")
            ok = False
            continue
        if c["size"] != r["size"]:
            print(f"  ✗ origin {key} size {c['size']} != ref {r['size']}")
            ok = False
        for fname, face in c["uv"].items():
            rf = r.get("uv", {}).get(fname)
            if rf is None:
                continue  # 参考可能裁掉了某些面
            if face["uv"] != rf["uv"] or face["uv_size"] != rf["uv_size"]:
                print(
                    f"  ✗ origin {key} face {fname} uv {face} != ref {rf}"
                )
                ok = False
    print(
        f"  oracle: 转换 {len(mine)} cube 对照 coffin_common.geo.json "
        f"{len(ref_cubes)} cube → {'PASS ✓' if ok else 'FAIL ✗'}"
    )
    return ok


# ---------------------------------------------------------------------------
def export_tier(tier: str) -> None:
    stem = TIERS[tier]
    bb = _load(MODELS / f"{stem}.bbmodel")
    ident = f"geometry.bong.{tier}_coffin"

    geo = build_geo(bb, ident)
    png, tw, th = extract_texture(bb)
    # 用实际 PNG 尺寸覆盖 texture_width/height（GeckoLib 硬约束）
    geo["minecraft:geometry"][0]["description"]["texture_width"] = tw
    geo["minecraft:geometry"][0]["description"]["texture_height"] = th
    anim = build_animation(tier)

    GEO_DIR.mkdir(parents=True, exist_ok=True)
    TEX_DIR.mkdir(parents=True, exist_ok=True)
    ANIM_DIR.mkdir(parents=True, exist_ok=True)

    geo_path = GEO_DIR / f"{tier}_coffin.geo.json"
    tex_path = TEX_DIR / f"{tier}_coffin_intact.png"
    anim_path = ANIM_DIR / f"{tier}_coffin.animation.json"
    geo_path.write_text(json.dumps(geo, indent=2, ensure_ascii=False) + "\n")
    tex_path.write_bytes(png)
    anim_path.write_text(json.dumps(anim, indent=2, ensure_ascii=False) + "\n")

    n_cubes = sum(len(b.get("cubes", [])) for b in geo["minecraft:geometry"][0]["bones"])
    bone_names = [b["name"] for b in geo["minecraft:geometry"][0]["bones"]]
    print(
        f"  [{tier}] {stem}.bbmodel → {n_cubes} cube / bones {bone_names} / tex {tw}×{th}"
    )
    print(f"      geo  → {geo_path.relative_to(REPO)}")
    print(f"      tex  → {tex_path.relative_to(REPO)}")
    print(f"      anim → {anim_path.relative_to(REPO)}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--verify", action="store_true", help="仅跑 CoffinCommon oracle 自检")
    ap.add_argument("--tier", choices=list(TIERS), help="只导出指定档")
    args = ap.parse_args()

    print("== CoffinCommon oracle 自检 ==")
    if not verify_oracle():
        raise SystemExit("oracle 校验失败：bbmodel→geo 转换关系不成立，已中止导出")
    if args.verify:
        return

    print("\n== 导出延寿棺资产 ==")
    tiers = [args.tier] if args.tier else list(TIERS)
    for tier in tiers:
        export_tier(tier)
    print("\n完成。进游戏实机比对核验渲染（或用本地预览脚本）。")


if __name__ == "__main__":
    main()
