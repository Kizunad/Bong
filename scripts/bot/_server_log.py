"""按文件偏移增量扫描 server 日志（guard 标记 / deserialize-failed 归属）。

bot-e2e.sh 用全 run 共享的 server 日志，随前面所有场景持续增长。护栏场景的
负向断言若每 200ms 从头全扫整个文件，窗口内几十次全量读会产生几十 GB I/O，
且单次扫描时长不受截止时间约束（review finding [minor]：guard 轮询反复全量
重扫无界日志）。本类每次只 seek 到上次读到的偏移读追加段；日志轮转/截断时
从头重读；未写完的末尾行回退到行起点，等补全后再处理。

deserialize-failed 归属按结构化 `user=<name>` 字段**精确**匹配，不做子串匹配
——两个重叠用户名（Throw / Throw2）不会互相误归因（review finding [minor]：
user 子串误归因）。
"""

from __future__ import annotations

import os
import re
from typing import Optional

_DESERIALIZE_FAILED_RE = re.compile(r"client_request deserialize failed")
# server 端 deserialize-failed warn 的 user= 字段（client_request_handler.rs）：
# `...(user=<username>): ...`。按完整字段值精确匹配归属，杜绝子串碰撞。
_USER_ATTR_RE = re.compile(r"\(user=([^)]+)\)")


class ServerLogScanner:
    """对 server 日志做增量扫描，累计 guard 标记与 deserialize-failed 归属。

    guard_re 须带 `carrier` 与 `reason` 两个命名分组（如 throw/container_switch
    的 guard 行）。scan() 只读上次偏移之后的追加段，调用方在轮询循环里反复
    scan() 不会重复解析旧行。
    """

    def __init__(self, path: str, guard_re: "re.Pattern[str]") -> None:
        self._path = path
        self._guard_re = guard_re
        self._offset = 0
        self._guards: list[tuple[str, str]] = []  # (carrier, reason)
        self._deserialize_failed_users: list[str] = []

    def _read_new(self) -> list[str]:
        if not os.path.isfile(self._path):
            return []
        size = os.path.getsize(self._path)
        if self._offset > size:
            # 日志轮转/截断：偏移失效，从新文件头重读。
            self._offset = 0
        with open(self._path, "r", encoding="utf-8", errors="replace") as fh:
            fh.seek(self._offset)
            data = fh.read()
            self._offset = fh.tell()
        if not data.endswith("\n"):
            # 末尾行可能只写了一半：把偏移回退到该行起点，等补全后再处理，
            # 否则行尾续写会被永久跳过。
            nl = data.rfind("\n")
            self._offset -= len(data) - (nl + 1)
            data = data[: nl + 1]
        return data.splitlines()

    def scan(self) -> None:
        """消费上次偏移以来的追加段，更新累计的 guard 标记与归属计数。"""
        for line in self._read_new():
            m = self._guard_re.search(line)
            if m:
                self._guards.append((m.group("carrier"), m.group("reason")))
            if _DESERIALIZE_FAILED_RE.search(line):
                um = _USER_ATTR_RE.search(line)
                if um:
                    self._deserialize_failed_users.append(um.group(1))

    def guard_markers(self, carrier: str) -> list[str]:
        """返回 scan() 已累计的、归属给定 carrier 的 guard reason 序列（有序）。"""
        return [reason for c, reason in self._guards if c == carrier]

    def deserialize_failed(self, username: str) -> int:
        """归属给定 username 的 deserialize-failed 精确计数（user= 字段完全相等）。"""
        return sum(1 for u in self._deserialize_failed_users if u == username)
