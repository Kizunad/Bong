"""项目根与产出目录的唯一解析处。

拆库前，`core/` 和 `tools/` 里到处是 `REPO = Path(__file__).resolve().parents[2]`
——写死了「本文件住在 <仓库根>/modelScript/core/ 下」。这个假设在库被搬进独立 repo
后必然崩：那时 parents[2] 是 site-packages 的某一层，跟调用方的项目根毫无关系。

`Workspace` 把「根在哪、产出往哪写、贴图按什么解析、资源命名空间叫什么」收成一处，
由调用方显式给或按约定发现，库自己不再猜。

发现顺序（先命中先用）：
  1. 显式传入的 root
  2. 环境变量 $BBMODEL_PROJECT_ROOT
  3. 从起点逐级向上找 bbmodel.toml（配置文件所在目录即根）
  4. 从起点逐级向上找 .git（**worktree 里 .git 是文件不是目录**，两种都认）
  5. 当前工作目录

用法：

    from workspace import Workspace

    ws = Workspace.discover()          # 按约定发现
    ws.models / "Workbench.bbmodel"    # 产出路径
    ws.rel(path)                       # 打印用的短路径
"""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ENV_ROOT = "BBMODEL_PROJECT_ROOT"
CONFIG_NAME = "bbmodel.toml"
DEFAULT_LIB_DIRNAME = "modelScript"
DEFAULT_NAMESPACE = "bong"


def _walk_up(start: Path):
    """从 start 所在目录逐级向上，含自身与全部祖先。"""
    resolved = start.resolve()
    base = resolved if resolved.is_dir() else resolved.parent
    yield base
    yield from base.parents


def _find_upwards(start: Path, name: str) -> Path | None:
    """向上找名为 name 的条目所在目录。`.git` 在 worktree 里是文件，故不限类型。"""
    for d in _walk_up(start):
        if (d / name).exists():
            return d
    return None


@dataclass(frozen=True)
class Workspace:
    """一次资产生产的落脚点。"""

    root: Path
    lib: Path
    namespace: str = DEFAULT_NAMESPACE
    client_resources: Path | None = None
    _extra: dict = field(default_factory=dict, repr=False, compare=False)

    # ── 派生目录 ─────────────────────────────────────────────
    @property
    def models(self) -> Path:
        return self.lib / "models"

    @property
    def out(self) -> Path:
        return self.lib / "out"

    @property
    def assets(self) -> Path:
        return self.lib / "assets"

    @property
    def manifests(self) -> Path:
        return self.lib / "manifests"

    # ── 调用方的客户端资源树（可选）─────────────────────────
    def require_client_resources(self) -> Path:
        """取 client 资源根，没配就报可执行的错。

        库不该猜调用方把 MC 资源树放在哪。没配时给出确切的补法，而不是让
        `None / "assets"` 在下游炸出一个看不懂的 TypeError。
        """
        if self.client_resources is None:
            raise RuntimeError(
                f"需要 client 资源树，但当前 workspace（根 {self.root}，来源 "
                f"{self.source}）没有配置它。在 {self.root / CONFIG_NAME} 里写："
                f'\n\n    client_resources = "client/src/main/resources"\n'
            )
        return self.client_resources

    @property
    def client_assets(self) -> Path:
        """`<client 资源根>/assets/<namespace>`。"""
        return self.require_client_resources() / "assets" / self.namespace

    @property
    def player_animations(self) -> Path:
        """PlayerAnimator 的动画 JSON 目录。"""
        return self.client_assets / "player_animation"

    # ── 发现 ─────────────────────────────────────────────────
    @classmethod
    def discover(
        cls,
        root: Path | str | None = None,
        *,
        start: Path | str | None = None,
        namespace: str | None = None,
    ) -> Workspace:
        start_path = Path(start) if start is not None else Path.cwd()

        if root is not None:
            resolved = Path(root).resolve()
            source = "explicit"
        elif os.environ.get(ENV_ROOT):
            resolved = Path(os.environ[ENV_ROOT]).resolve()
            source = "env"
        else:
            found = _find_upwards(start_path, CONFIG_NAME)
            source = "config"
            if found is None:
                found = _find_upwards(start_path, ".git")
                source = "git"
            if found is None:
                found = Path.cwd().resolve()
                source = "cwd"
            resolved = found

        config = cls._read_config(resolved)
        lib = resolved / config.get("lib", DEFAULT_LIB_DIRNAME)
        ns = (
            namespace
            if namespace is not None
            else config.get("namespace", DEFAULT_NAMESPACE)
        )
        client = config.get("client_resources")
        return cls(
            root=resolved,
            lib=lib,
            namespace=ns,
            client_resources=(resolved / client) if client else None,
            _extra={"source": source},
        )

    @staticmethod
    def _read_config(root: Path) -> dict:
        path = root / CONFIG_NAME
        if not path.is_file():
            return {}
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        section = data.get("workspace", data)
        return section if isinstance(section, dict) else {}

    @property
    def source(self) -> str:
        """根是怎么定下来的：explicit / env / config / git / cwd。用于排错。"""
        return self._extra.get("source", "explicit")

    # ── 路径服务 ─────────────────────────────────────────────
    def rel(self, path: Path | str) -> str:
        """打印用的短路径：能相对 root 就相对，不能就给绝对路径。"""
        p = Path(path)
        try:
            return str(p.resolve().relative_to(self.root))
        except ValueError:
            return str(p)

    def resolve_texture(self, src: str, near: Path | str | None = None) -> Path:
        """解析 bbmodel 贴图的 source 路径。

        bbmodel 里存的多是仓库相对路径（`client/src/main/...`），也可能是相对
        bbmodel 自身的路径或绝对路径。三种都试，都不中就带上全部候选报错——
        排错时最想知道的就是「到底找了哪几个地方」。
        """
        candidates = [self.root / src]
        if near is not None:
            candidates.append(Path(near).resolve().parent / src)
        candidates.append(Path(src))
        for c in candidates:
            if c.is_file():
                return c
        raise FileNotFoundError(
            f"贴图路径 {src!r} 在以下位置都找不到: "
            + ", ".join(str(c) for c in candidates)
        )


_DEFAULT: Workspace | None = None


def default() -> Workspace:
    """进程内共享的默认 workspace（首次调用时发现，之后复用）。"""
    global _DEFAULT
    if _DEFAULT is None:
        _DEFAULT = Workspace.discover()
    return _DEFAULT


def set_default(ws: Workspace | None) -> None:
    """显式覆盖默认 workspace；传 None 清掉（测试用）。"""
    global _DEFAULT
    _DEFAULT = ws
