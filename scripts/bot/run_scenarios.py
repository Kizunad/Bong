#!/usr/bin/env python3
"""Bot e2e 场景 runner —— CI 由 scripts/bot-e2e.sh 调用，本地也可直连已起的 server。

用法：
    python3 scripts/bot/run_scenarios.py --list
    python3 scripts/bot/run_scenarios.py --all                 # 默认
    python3 scripts/bot/run_scenarios.py --scenario cmd_dev_give_feedback
    python3 scripts/bot/run_scenarios.py --host 192.168.0.108 --port 25565

退出码：0 = 全绿；1 = 有失败/错误；2 = 用法/环境错误（连不上 server 等）。
"""

from __future__ import annotations

import argparse
import importlib
import os
import pkgutil
import socket
import sys
import time
import traceback

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bot.bot import Bot, BotAssertionError  # noqa: E402


class ScenarioEnv:
    """传给场景 run(env) 的句柄：负责造唯一用户名的 bot。"""

    def __init__(self, host: str, port: int, run_tag: str):
        self.host = host
        self.port = port
        self.run_tag = run_tag

    def new_bot(self, tag: str) -> Bot:
        username = f"B{self.run_tag}{tag}"
        if len(username) > 16:
            raise ValueError(
                f"用户名 {username!r} 超 16 字符上限——run_tag({self.run_tag!r}) 或场景 tag({tag!r}) 太长"
            )
        return Bot(username, host=self.host, port=self.port)

    def lookup_character_id(self, username: str) -> str:
        """反查 server 侧当前角色的完整 character_id（`offline:<user>:<uuid>`）。

        运行时 lifecycle.character_id 是复合形式（`player_character_id` = canonical id +
        `player_core.current_char_id` uuid），且任何 S2C payload 都不下发该 uuid——
        duo_she 客户端 UI 尚未接线，黑盒客户端无法自行构造合法 target_id。
        本查询仅用于**构造输入**（同 fixture raster 由 harness 提供），断言面仍是纯协议黑盒。
        """
        import sqlite3

        candidates = [
            os.environ.get("BONG_SERVER_DB"),
            os.path.join("data", "bong.db"),
            os.path.join("server", "data", "bong.db"),
        ]
        tried = []
        for path in candidates:
            if not path or not os.path.isfile(path):
                continue
            tried.append(path)
            try:
                connection = sqlite3.connect(path, timeout=10.0)
                try:
                    row = connection.execute(
                        "SELECT current_char_id FROM player_core WHERE username = ?1",
                        (username,),
                    ).fetchone()
                finally:
                    connection.close()
            except sqlite3.Error as error:
                raise RuntimeError(
                    f"lookup_character_id(`{username}`) 读 {path} 失败: {error}"
                ) from error
            if row is not None:
                return f"offline:{username}:{row[0]}"
        raise RuntimeError(
            f"lookup_character_id(`{username}`) 查不到 player_core 行——"
            f"尝试过 {tried or [p for p in candidates if p]}"
        )


def validate_scenario_module(name: str, module: object) -> None:
    """校验场景模块契约（DESCRIPTION/MODULES/run），缺失抛 RuntimeError。"""
    for attr in ("DESCRIPTION", "MODULES", "run"):
        if not hasattr(module, attr):
            raise RuntimeError(
                f"场景 {name} 缺少 {attr} —— 见 scripts/bot/scenarios/__init__.py 的模块契约"
            )


def discover_scenarios() -> dict[str, object]:
    import bot.scenarios as scenarios_pkg

    found = {}
    for info in sorted(pkgutil.iter_modules(scenarios_pkg.__path__), key=lambda m: m.name):
        if info.name.startswith("_"):
            continue
        module = importlib.import_module(f"bot.scenarios.{info.name}")
        validate_scenario_module(info.name, module)
        found[info.name] = module
    return found


def check_server_reachable(host: str, port: int, timeout: float) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=os.environ.get("BOT_E2E_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("BOT_E2E_PORT", "25565")))
    parser.add_argument("--list", action="store_true", help="只列出场景不执行")
    parser.add_argument(
        "--scenario",
        action="append",
        default=None,
        help="只跑指定场景（可重复传）；缺省跑全部",
    )
    parser.add_argument("--all", action="store_true", help="跑全部场景（默认行为，显式起见）")
    parser.add_argument(
        "--run-tag",
        default=os.environ.get("BOT_E2E_RUN_TAG", str(os.getpid() % 100000)),
        help="用户名区分段（同一 server 反复跑时避免脏状态叠加），≤5 字符",
    )
    args = parser.parse_args()

    scenarios = discover_scenarios()

    if args.list:
        for name, module in scenarios.items():
            default = "default" if getattr(module, "DEFAULT_ENABLED", True) else "dedicated"
            print(
                f"{name:44} modules={','.join(module.MODULES):20} "
                f"mode={default:9} {module.DESCRIPTION}"
            )
        return 0

    selected = args.scenario or list(scenarios)
    unknown = [name for name in selected if name not in scenarios]
    if unknown:
        print(f"未知场景: {unknown}；可用: {list(scenarios)}", file=sys.stderr)
        return 2

    if not check_server_reachable(args.host, args.port, timeout=5.0):
        print(
            f"server {args.host}:{args.port} 不可达——先起 server"
            "（bash scripts/bot-e2e.sh 会自动起，或手动 scripts/build-token.sh cargo run）",
            file=sys.stderr,
        )
        return 2

    env = ScenarioEnv(args.host, args.port, args.run_tag)
    results: list[tuple[str, str, str]] = []
    for name in selected:
        module = scenarios[name]
        print(f"\n=== scenario: {name} ===\n    {module.DESCRIPTION}")
        default_enabled = getattr(module, "DEFAULT_ENABLED", True)
        required_env = getattr(module, "REQUIRED_ENV", None)
        run_in_all_when_env = getattr(module, "RUN_IN_ALL_WHEN_ENV", None)
        enabled_by_env = (
            run_in_all_when_env is not None
            and os.environ.get(run_in_all_when_env) == "1"
        )
        if args.scenario is None and not default_enabled and not enabled_by_env:
            reason = (
                f"专用场景；常规 --all 仅在 {run_in_all_when_env}=1 时执行"
                if run_in_all_when_env is not None
                else "专用场景；常规 --all 不执行（需显式 --scenario）"
            )
            results.append((name, "SKIP", reason))
            print(f"    SKIP: {reason}")
            continue
        if args.scenario is not None and required_env is not None and os.environ.get(required_env) != "1":
            reason = f"专用场景；显式 --scenario 需 {required_env}=1"
            results.append((name, "ERROR", reason))
            print(f"    ERROR: {reason}")
            continue
        started = time.monotonic()
        try:
            module.run(env)
        except BotAssertionError as failure:
            results.append((name, "FAIL", str(failure)))
            print(f"    FAIL ({time.monotonic() - started:.1f}s): {failure}")
        except Exception:
            trace = traceback.format_exc()
            results.append((name, "ERROR", trace))
            print(f"    ERROR ({time.monotonic() - started:.1f}s):\n{trace}")
        else:
            results.append((name, "PASS", ""))
            print(f"    PASS ({time.monotonic() - started:.1f}s)")

    print("\n===== bot e2e summary =====")
    for name, status, _ in results:
        print(f"  {status:5}  {name}")
    failed = [result for result in results if result[1] in {"FAIL", "ERROR"}]
    passed = [result for result in results if result[1] == "PASS"]
    skipped = [result for result in results if result[1] == "SKIP"]
    print(
        f"  total={len(results)} pass={len(passed)} skip={len(skipped)} fail={len(failed)}"
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
