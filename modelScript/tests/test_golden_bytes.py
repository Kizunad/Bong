"""golden 字节回归：生成器的产出不许在重构中漂。

拆库重构（plan-modelscript-split-v1）会动 core/ 的 import 路径、路径解析和渲染底座，
这些改动**不该改变任何一件资产的几何 / UV / 贴图**。这里把「今天生成器产出什么」
钉成 fixture，重构前后各跑一次，一个字节都不许变。

比对前先把 uuid 归一化：bbmodel 的 uuid 只是身份句柄，多数生成器用 `uuid.uuid4()`
现取，每跑一次都不同。归一化后剩下的全是真内容。

跑一轮约 30 秒（40 个生成器 × 各开一个独立沙箱）。
fixture 需要重录时：`python3 modelScript/tests/test_golden_bytes.py --record`
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import golden_runner  # noqa: E402

FIXTURE = Path(__file__).resolve().parent / "fixtures" / "golden_bbmodel.json"


def load_fixture() -> dict[str, dict]:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def record_fixture() -> None:
    data = golden_runner.collect(
        progress=lambda name, r: print(
            f"  {name:34s} exit={r['exit']}  产出 {len(r['outputs'])}", flush=True
        )
    )
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(
        json.dumps(data, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"→ {FIXTURE}  ({len(data)} 个生成器)")


class NormalizedHashTest(unittest.TestCase):
    """归一化本身要先立得住，否则整套 golden 是假的。"""

    def test_uuid_difference_alone_does_not_change_hash(self) -> None:
        a = b'{"uuid": "94f5cbe3-1b56-4b2f-87fb-4233954c1464", "from": [0, 0, 0]}'
        b = b'{"uuid": "1a11fe52-374d-4d65-927e-7bdca52852f1", "from": [0, 0, 0]}'
        self.assertEqual(
            golden_runner.normalized_hash(a),
            golden_runner.normalized_hash(b),
            "期望：只有 uuid 不同的两份内容归一化后同哈希（uuid 是身份句柄不是内容）；"
            "实际：哈希不同，说明归一化没盖住 uuid",
        )

    def test_geometry_difference_changes_hash(self) -> None:
        a = b'{"uuid": "94f5cbe3-1b56-4b2f-87fb-4233954c1464", "from": [0, 0, 0]}'
        b = b'{"uuid": "94f5cbe3-1b56-4b2f-87fb-4233954c1464", "from": [0, 0, 1]}'
        self.assertNotEqual(
            golden_runner.normalized_hash(a),
            golden_runner.normalized_hash(b),
            "期望：几何变了哈希必须变，否则 golden 挡不住真回归；实际：哈希相同",
        )

    def test_uuid_identity_structure_is_preserved(self) -> None:
        """同一个 uuid 出现两次 vs 两个不同 uuid，必须区分开。

        bbmodel 的 outliner 靠裸 uuid 字符串引用 cube。若归一化把所有 uuid 抹成同一个
        符号，「A 引用 A」和「A 引用 B」就无法区分，接线错误会从 golden 底下溜过去。
        """
        same = b'["11111111-1111-4111-8111-111111111111", "11111111-1111-4111-8111-111111111111"]'
        diff = b'["11111111-1111-4111-8111-111111111111", "22222222-2222-4222-8222-222222222222"]'
        self.assertNotEqual(
            golden_runner.normalized_hash(same),
            golden_runner.normalized_hash(diff),
            "期望：uuid 的引用结构（谁和谁是同一个）在归一化后保留；"
            "实际：同 uuid 重复与两个不同 uuid 被归一成一样，outliner 接线错误会漏网",
        )

    def test_non_uuid_hex_is_not_mangled(self) -> None:
        """贴图是 base64，里面难免出现像 uuid 的十六进制片段——不能误伤。"""
        payload = b'{"source": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg"}'
        self.assertEqual(
            golden_runner.normalized_hash(payload),
            golden_runner.normalized_hash(payload),
            "期望：同一份内容两次归一化同哈希；实际：不稳定",
        )


class GoldenBytesTest(unittest.TestCase):
    """跑一遍全部生成器，和 fixture 对拍。"""

    collected: dict[str, dict]
    fixture: dict[str, dict]

    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = load_fixture()
        cls.collected = golden_runner.collect()

    def test_generator_set_matches_fixture(self) -> None:
        missing = sorted(set(self.fixture) - set(self.collected))
        extra = sorted(set(self.collected) - set(self.fixture))
        self.assertEqual(
            (missing, extra),
            ([], []),
            f"期望：fixture 覆盖的生成器集合与磁盘上的一致。"
            f"实际：fixture 有而磁盘没有 {missing}；磁盘有而 fixture 没有 {extra}。"
            f"新增生成器要重录 fixture：python3 {Path(__file__).name} --record",
        )

    def test_exit_codes_match(self) -> None:
        for name in sorted(set(self.fixture) & set(self.collected)):
            with self.subTest(generator=name):
                self.assertEqual(
                    self.collected[name]["exit"],
                    self.fixture[name]["exit"],
                    f"期望：{name} 在干净沙箱里的退出码是 {self.fixture[name]['exit']}"
                    f"（fixture 记录值），实际 {self.collected[name]['exit']}。"
                    f"退出码变 1 常见原因：往 gitignored 的 out/ 落盘前漏了 "
                    f"mkdir(parents=True, exist_ok=True)",
                )

    def test_outputs_match(self) -> None:
        for name in sorted(set(self.fixture) & set(self.collected)):
            want = self.fixture[name]["outputs"]
            got = self.collected[name]["outputs"]
            with self.subTest(generator=name):
                self.assertEqual(
                    sorted(want),
                    sorted(got),
                    f"期望：{name} 写出 {sorted(want)}，实际写出 {sorted(got)}。"
                    f"产出文件集合变了 = 生成器的落盘路径或条件被改动了",
                )
                for rel in sorted(set(want) & set(got)):
                    self.assertEqual(
                        got[rel],
                        want[rel],
                        f"期望：{name} 产出的 {rel} 归一化哈希是 {want[rel][:16]}…"
                        f"（fixture 记录值），实际 {got[rel][:16]}…。"
                        f"uuid 已归一化，所以这是**真内容变了**：几何 / UV / 内嵌贴图。"
                        f"重构不该改内容；确认是有意改动才重录 fixture",
                    )


if __name__ == "__main__":
    if "--record" in sys.argv:
        record_fixture()
    else:
        unittest.main()
