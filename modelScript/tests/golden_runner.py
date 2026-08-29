"""golden 回归底座：在沙箱里跑生成器，产出 uuid 归一化后的内容哈希。

为什么要归一化 uuid：bbmodel 的 uuid 只是身份句柄，多数生成器用 `uuid.uuid4()` 现取，
每次跑都不同（Blockbench 打开也会重排）。归一化后几何 / UV / 内嵌 base64 贴图逐字节可比。

为什么要沙箱：生成器直接写 `modelScript/models/`。在真树里跑会把已入库资产改脏，
其中三件延寿棺还是 Blockbench 手改稿（fmt 5.0），被生成器的 4.10 覆盖就找不回来了。
沙箱按 `git ls-files` 复制**已跟踪文件**，跑完即扔。
"""

from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
# 沙箱要带上两块 client 资源：
#   client/tools —— core/render_player_pose.py 会把它插进 sys.path
#   player_animation —— gen_*_player_anim.py 从这里读 PlayerAnimator JSON 当输入
SANDBOX_TREES = (
    "modelScript",
    "client/tools",
    "client/src/main/resources/assets/bong/player_animation",
)

_UUID = re.compile(rb"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


def normalized_hash(data: bytes) -> str:
    """把出现过的 uuid 按首次出现顺序换成序号，再取 sha256。"""
    seen: dict[bytes, bytes] = {}

    def _replace(match: re.Match[bytes]) -> bytes:
        key = match.group(0)
        if key not in seen:
            seen[key] = b"UUID%04d" % len(seen)
        return seen[key]

    return hashlib.sha256(_UUID.sub(_replace, data)).hexdigest()


def tracked_files(tree: str) -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z", tree],
        capture_output=True, check=True,
    ).stdout
    return [p for p in out.decode().split("\0") if p]


def build_sandbox(dest: Path) -> Path:
    """把已跟踪文件按原相对路径复制到 dest，返回 dest。"""
    for tree in SANDBOX_TREES:
        for rel in tracked_files(tree):
            src = REPO / rel
            if not src.exists():          # 只在索引里、工作区没有的跳过
                continue
            target = dest / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, target)
    return dest


def generator_names() -> list[str]:
    return sorted(p.name for p in (REPO / "modelScript/generators").glob("gen_*.py"))


def run_generator(sandbox: Path, name: str, timeout: int = 900) -> int:
    proc = subprocess.run(
        [sys.executable, str(sandbox / "modelScript/generators" / name)],
        capture_output=True, text=True, timeout=timeout, cwd=str(sandbox),
    )
    return proc.returncode


def snapshot_models(sandbox: Path) -> dict[str, tuple[int, str]]:
    """{相对路径: (mtime_ns, 归一化哈希)}。

    mtime 用来判「这个生成器写没写它」——多数生成器是确定性的，重写同样的内容
    哈希不会变，只看哈希会把它们全判成「零产出」。
    """
    root = sandbox / "modelScript/models"
    if not root.is_dir():
        return {}
    return {
        str(p.relative_to(root)): (p.stat().st_mtime_ns, normalized_hash(p.read_bytes()))
        for p in sorted(root.rglob("*"))
        if p.is_file()
    }


def collect(progress=None) -> dict[str, dict]:
    """逐个生成器**各开一个全新沙箱**跑，记录退出码 + 写出的 models 文件哈希。

    必须逐个隔离，不能共用一个沙箱：`modelScript/out/` 不入库，谁先跑谁把它建出来，
    共用沙箱会让「落盘前没 mkdir」的生成器搭上前一个的便车——实测 8 个生成器的
    这类缺陷就是被执行顺序掩盖的，共用沙箱下只暴露 2 个。
    """
    import tempfile

    result: dict[str, dict] = {}
    for name in generator_names():
        with tempfile.TemporaryDirectory(prefix="bbmodel-golden-") as td:
            sandbox = build_sandbox(Path(td))
            before = snapshot_models(sandbox)
            code = run_generator(sandbox, name)
            after = snapshot_models(sandbox)
        outputs = {
            rel: entry[1]
            for rel, entry in sorted(after.items())
            if rel not in before or before[rel][0] != entry[0]
        }
        result[name] = {"exit": code, "outputs": outputs}
        if progress is not None:
            progress(name, result[name])
    return result
