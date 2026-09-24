"""Workspace 的饱和测试。

这是拆库的地基：库搬进独立 repo 后，「项目根在哪」不再能靠 `parents[2]` 猜，
全靠这里的发现顺序。发现错根 = 贴图找不到 + 产出写错地方，且多半静默。
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker import workspace  # noqa: E402
from bbmodel_maker.workspace import Workspace  # noqa: E402


class DiscoveryPrecedenceTest(unittest.TestCase):
    """发现顺序：显式 > 环境变量 > bbmodel.toml > .git > cwd。"""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="ws-")
        self.tmp = Path(self._tmp.name).resolve()
        self._saved_env = os.environ.pop(workspace.ENV_ROOT, None)
        self.addCleanup(self._tmp.cleanup)
        self.addCleanup(self._restore_env)

    def _restore_env(self) -> None:
        os.environ.pop(workspace.ENV_ROOT, None)
        if self._saved_env is not None:
            os.environ[workspace.ENV_ROOT] = self._saved_env

    def _mk(self, rel: str, content: str = "") -> Path:
        p = self.tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return p

    def test_explicit_root_beats_env(self) -> None:
        explicit = self.tmp / "explicit"
        explicit.mkdir()
        os.environ[workspace.ENV_ROOT] = str(self.tmp / "from-env")
        ws = Workspace.discover(root=explicit)
        self.assertEqual(
            ws.root, explicit,
            f"期望：显式传入的 root 优先级最高，因为调用方最清楚自己要什么；"
            f"实际根落在 {ws.root}（来源 {ws.source}）",
        )
        self.assertEqual(ws.source, "explicit")

    def test_env_beats_config(self) -> None:
        self._mk("bbmodel.toml", "")
        env_root = self.tmp / "from-env"
        env_root.mkdir()
        os.environ[workspace.ENV_ROOT] = str(env_root)
        ws = Workspace.discover(start=self.tmp)
        self.assertEqual(
            ws.root, env_root,
            f"期望：$BBMODEL_PROJECT_ROOT 压过配置文件发现（CI / 沙箱靠它顶掉约定）；"
            f"实际 {ws.root}（来源 {ws.source}）",
        )
        self.assertEqual(ws.source, "env")

    def test_config_beats_git(self) -> None:
        self._mk(".git", "")
        self._mk("nested/bbmodel.toml", "")
        ws = Workspace.discover(start=self.tmp / "nested" / "deep" / "deeper")
        self.assertEqual(
            ws.root, self.tmp / "nested",
            f"期望：bbmodel.toml 比 .git 近就用它——一个 git 仓库里可以有多个项目根；"
            f"实际 {ws.root}（来源 {ws.source}）",
        )
        self.assertEqual(ws.source, "config")

    def test_git_directory_is_found(self) -> None:
        (self.tmp / ".git").mkdir()
        (self.tmp / "a" / "b").mkdir(parents=True)
        ws = Workspace.discover(start=self.tmp / "a" / "b")
        self.assertEqual(ws.root, self.tmp)
        self.assertEqual(ws.source, "git")

    def test_git_file_is_found(self) -> None:
        """worktree 里 `.git` 是文件不是目录——只认目录会一路找到主仓库去。"""
        self._mk(".git", "gitdir: /somewhere/.git/worktrees/x\n")
        (self.tmp / "a").mkdir()
        ws = Workspace.discover(start=self.tmp / "a")
        self.assertEqual(
            ws.root, self.tmp,
            f"期望：.git 是文件（git worktree 的形态）时同样认作根；"
            f"实际 {ws.root}（来源 {ws.source}）——只认目录会越过 worktree 找到主仓库",
        )
        self.assertEqual(ws.source, "git")

    def test_start_may_be_a_file(self) -> None:
        """起点常是 `__file__`，是文件不是目录。"""
        self._mk(".git", "")
        f = self._mk("a/b/gen_thing.py", "# ...")
        ws = Workspace.discover(start=f)
        self.assertEqual(ws.root, self.tmp)

    def test_cwd_fallback_when_nothing_found(self) -> None:
        ws = Workspace.discover(start=self.tmp)
        self.assertEqual(
            ws.source, "cwd",
            f"期望：既无配置也无 .git 时退回 cwd 并如实标注来源；实际来源 {ws.source}",
        )
        self.assertEqual(ws.root, Path.cwd().resolve())


class ConfigTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="ws-cfg-")
        self.tmp = Path(self._tmp.name).resolve()
        self.addCleanup(self._tmp.cleanup)

    def _config(self, body: str) -> Workspace:
        (self.tmp / "bbmodel.toml").write_text(body, encoding="utf-8")
        return Workspace.discover(start=self.tmp)

    def test_defaults_without_config(self) -> None:
        ws = Workspace.discover(root=self.tmp)
        self.assertEqual(ws.lib, self.tmp / workspace.DEFAULT_LIB_DIRNAME)
        self.assertEqual(ws.namespace, workspace.DEFAULT_NAMESPACE)
        self.assertIsNone(ws.client_resources)

    def test_workspace_section(self) -> None:
        ws = self._config('[workspace]\nlib = "assets3d"\nnamespace = "acme"\n')
        self.assertEqual(ws.lib, self.tmp / "assets3d")
        self.assertEqual(ws.namespace, "acme")

    def test_flat_config_without_section(self) -> None:
        ws = self._config('lib = "flat"\nnamespace = "flatns"\n')
        self.assertEqual(
            (ws.lib, ws.namespace), (self.tmp / "flat", "flatns"),
            "期望：不带 [workspace] 段的扁平配置同样生效（少打一行是常见写法）",
        )

    def test_client_resources_is_root_relative(self) -> None:
        ws = self._config('client_resources = "client/src/main/resources"\n')
        self.assertEqual(ws.client_resources, self.tmp / "client/src/main/resources")

    def test_namespace_argument_beats_config(self) -> None:
        (self.tmp / "bbmodel.toml").write_text('namespace = "from-config"\n', encoding="utf-8")
        ws = Workspace.discover(start=self.tmp, namespace="from-arg")
        self.assertEqual(
            ws.namespace, "from-arg",
            "期望：显式传的 namespace 压过配置；实际取了配置值",
        )

    def test_empty_config_falls_back_to_defaults(self) -> None:
        ws = self._config("")
        self.assertEqual(ws.namespace, workspace.DEFAULT_NAMESPACE)
        self.assertEqual(ws.lib, self.tmp / workspace.DEFAULT_LIB_DIRNAME)


class DerivedDirectoriesTest(unittest.TestCase):
    def test_all_derived_from_lib(self) -> None:
        ws = Workspace(root=Path("/proj"), lib=Path("/proj/modelScript"))
        self.assertEqual(ws.models, Path("/proj/modelScript/models"))
        self.assertEqual(ws.out, Path("/proj/modelScript/out"))
        self.assertEqual(ws.assets, Path("/proj/modelScript/assets"))
        self.assertEqual(ws.manifests, Path("/proj/modelScript/manifests"))

    def test_lib_may_sit_outside_root(self) -> None:
        """库被 pip 装走后，调用方的 lib 目录不一定在 root 底下。"""
        ws = Workspace(root=Path("/proj"), lib=Path("/elsewhere/assets"))
        self.assertEqual(ws.models, Path("/elsewhere/assets/models"))


class RelTest(unittest.TestCase):
    def setUp(self) -> None:
        self.ws = Workspace(root=Path("/proj"), lib=Path("/proj/modelScript"))

    def test_inside_root_is_relative(self) -> None:
        self.assertEqual(self.ws.rel(Path("/proj/modelScript/out/a.png")),
                         "modelScript/out/a.png")

    def test_outside_root_stays_absolute(self) -> None:
        got = self.ws.rel(Path("/somewhere/else/a.png"))
        self.assertEqual(
            got, "/somewhere/else/a.png",
            f"期望：root 之外的路径原样给绝对路径（截断会让人以为文件在项目里）；实际 {got}",
        )

    def test_root_itself(self) -> None:
        self.assertEqual(self.ws.rel(Path("/proj")), ".")


class ResolveTextureTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="ws-tex-")
        self.tmp = Path(self._tmp.name).resolve()
        self.addCleanup(self._tmp.cleanup)
        self.ws = Workspace(root=self.tmp, lib=self.tmp / "modelScript")

    def _touch(self, rel: str) -> Path:
        p = self.tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(b"\x89PNG")
        return p

    def test_root_relative_hit(self) -> None:
        want = self._touch("client/src/main/resources/tex.png")
        self.assertEqual(
            self.ws.resolve_texture("client/src/main/resources/tex.png"), want,
            "期望：bbmodel 里存的仓库相对路径按 root 解析（这是最常见的一种）",
        )

    def test_near_model_relative_hit(self) -> None:
        model = self._touch("modelScript/models/Thing.bbmodel")
        want = self._touch("modelScript/models/Thing.png")
        self.assertEqual(
            self.ws.resolve_texture("Thing.png", near=model), want,
            "期望：root 下找不到时，退回按 bbmodel 自身所在目录解析",
        )

    def test_absolute_path_hit(self) -> None:
        want = self._touch("anywhere/abs.png")
        self.assertEqual(self.ws.resolve_texture(str(want)), want)

    def test_root_relative_wins_over_near(self) -> None:
        """两处都有同名文件时，仓库相对的那份优先——顺序不能反。"""
        root_hit = self._touch("dup.png")
        model = self._touch("modelScript/models/M.bbmodel")
        self._touch("modelScript/models/dup.png")
        self.assertEqual(
            self.ws.resolve_texture("dup.png", near=model), root_hit,
            "期望：候选顺序是 root → bbmodel 同目录 → 原样；实际优先取了同目录那份",
        )

    def test_miss_lists_every_candidate(self) -> None:
        model = self.tmp / "modelScript/models/M.bbmodel"
        with self.assertRaises(FileNotFoundError) as cm:
            self.ws.resolve_texture("nope.png", near=model)
        msg = str(cm.exception)
        for expected in [str(self.tmp / "nope.png"),
                         str(self.tmp / "modelScript/models/nope.png"),
                         "nope.png"]:
            self.assertIn(
                expected, msg,
                f"期望：找不到时把**全部**候选位置写进报错——排错时最想知道的就是"
                f"「到底找了哪几个地方」；实际报错里缺 {expected}：{msg}",
            )

    def test_miss_without_near_still_raises(self) -> None:
        with self.assertRaises(FileNotFoundError):
            self.ws.resolve_texture("nope.png")


class DefaultSingletonTest(unittest.TestCase):
    def setUp(self) -> None:
        self._saved = workspace._DEFAULT
        self.addCleanup(lambda: workspace.set_default(self._saved))

    def test_default_is_cached(self) -> None:
        workspace.set_default(None)
        first = workspace.default()
        self.assertIs(
            workspace.default(), first,
            "期望：默认 workspace 只发现一次然后复用（每次重新向上找文件系统是浪费）",
        )

    def test_set_default_overrides(self) -> None:
        forced = Workspace(root=Path("/forced"), lib=Path("/forced/lib"))
        workspace.set_default(forced)
        self.assertIs(workspace.default(), forced)

    def test_set_default_none_triggers_rediscovery(self) -> None:
        workspace.set_default(Workspace(root=Path("/x"), lib=Path("/x/lib")))
        workspace.set_default(None)
        self.assertNotEqual(
            workspace.default().root, Path("/x"),
            "期望：set_default(None) 清掉缓存并重新发现；实际还拿着旧的",
        )


if __name__ == "__main__":
    unittest.main()
