#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import io
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch

CHECKER = Path(__file__).resolve().parents[1] / "check_c2s_gate_matrix.py"
spec = importlib.util.spec_from_file_location("check_c2s_gate_matrix", CHECKER)
assert spec and spec.loader
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

ENUM_PREFIX = '#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]\n'


class ParserTests(unittest.TestCase):
    def test_accepts_struct_unit_and_tuple_variants(self) -> None:
        source = ENUM_PREFIX + """pub enum ClientRequestV1 {
    StructVariant {
        value: u8,
    },
    UnitVariant, SameLineVariant,
    TupleVariant(u8),
}
"""
        self.assertEqual(
            checker.parse_enum_variants(source),
            ["StructVariant", "UnitVariant", "SameLineVariant", "TupleVariant"],
        )

    def test_accepts_multiline_tuple_variant(self) -> None:
        source = ENUM_PREFIX + """pub enum ClientRequestV1 {
    TupleVariant(
        u8,
        String,
    ),
}
"""
        self.assertEqual(checker.parse_enum_variants(source), ["TupleVariant"])

    def test_accepts_multiline_tuple_variant_with_separate_trailing_comma(self) -> None:
        source = ENUM_PREFIX + """pub enum ClientRequestV1 {
    TupleVariant(
        u8,
    )
    ,
}
"""
        self.assertEqual(checker.parse_enum_variants(source), ["TupleVariant"])

    def test_fails_closed_on_unknown_top_level_syntax(self) -> None:
        sources = [
            ENUM_PREFIX + "pub enum ClientRequestV1 { #[cfg(test)] Unsupported = 1, }",
            ENUM_PREFIX + "pub enum ClientRequestV1 { TupleVariant(u8) = 1, }",
            ENUM_PREFIX + "pub enum ClientRequestV1 { StructVariant { value: u8 } = 1, }",
        ]
        for source in sources:
            with self.subTest(source=source), self.assertRaisesRegex(
                RuntimeError, "unsupported ClientRequestV1 variant syntax"
            ):
                checker.parse_enum_variants(source)

    def test_fails_closed_on_serde_wire_renames(self) -> None:
        camel = '#[serde(tag = "type", rename_all = "camelCase")]\npub enum ClientRequestV1 { Variant, }'
        with self.assertRaisesRegex(RuntimeError, 'rename_all = "snake_case"'):
            checker.parse_enum_variants(camel)

        renamed = ENUM_PREFIX + """pub enum ClientRequestV1 {
    #[serde(rename = "other")]
    Variant,
}
"""
        with self.assertRaisesRegex(RuntimeError, "variant-level serde rename"):
            checker.parse_enum_variants(renamed)

        stacked = ENUM_PREFIX + """pub enum ClientRequestV1 {
    #[serde(rename = "other")]
    #[doc = "variant docs"]
    Variant,
}
"""
        with self.assertRaisesRegex(RuntimeError, "variant-level serde rename"):
            checker.parse_enum_variants(stacked)

        inline = ENUM_PREFIX + """pub enum ClientRequestV1 {
    #[serde(rename = "other")] Variant,
}
"""
        with self.assertRaisesRegex(RuntimeError, "variant-level serde rename"):
            checker.parse_enum_variants(inline)

        inline_cfg = ENUM_PREFIX + """pub enum ClientRequestV1 {
    #[cfg(test)] Variant,
}
"""
        self.assertEqual(checker.parse_enum_variants(inline_cfg), ["Variant"])

    def test_accepts_valid_serde_contract_from_original_source(self) -> None:
        source = ENUM_PREFIX + "#[derive(Debug)]\npub enum ClientRequestV1 { Variant, }"
        self.assertEqual(checker.parse_enum_variants(source), ["Variant"])

    def test_serde_contract_uses_real_enum_not_decoy_text(self) -> None:
        source = """// #[serde(tag = \"type\", rename_all = \"snake_case\")] pub enum ClientRequestV1 { Decoy, }
/* outer comment
/* inner comment */
#[serde(tag = \"type\", rename_all = \"snake_case\")]
pub enum ClientRequestV1 { Decoy, }
*/
const DECOY: &str = \"#[serde(tag = \\\"type\\\", rename_all = \\\"snake_case\\\")] pub enum ClientRequestV1 { Decoy, }\";
#[serde(tag = \"kind\", rename_all = \"snake_case\")]
pub enum ClientRequestV1 {
    Variant,
}
"""
        with self.assertRaisesRegex(RuntimeError, 'tag = "type"'):
            checker.parse_enum_variants(source)

    def test_main_rejects_each_contract_drift(self) -> None:
        baseline = {
            "enum_variants": ["Alpha", "Beta"],
            "matrix_variants": ([1, 2], ["Alpha", "Beta"]),
        }
        cases = {
            "mismatched sets": {"matrix_variants": ([1, 2], ["Alpha", "Gamma"])},
            "reordered variants": {"matrix_variants": ([1, 2], ["Beta", "Alpha"])},
            "duplicate enum variants": {"enum_variants": ["Alpha", "Alpha"]},
            "duplicate matrix variants": {"matrix_variants": ([1, 2], ["Alpha", "Alpha"])},
            "non-contiguous rows": {"matrix_variants": ([1, 3], ["Alpha", "Beta"])},
        }
        for label, override in cases.items():
            values = baseline | override
            with self.subTest(label=label), patch.object(
                checker, "enum_variants", return_value=values["enum_variants"]
            ), patch.object(
                checker, "matrix_variants", return_value=values["matrix_variants"]
            ), redirect_stderr(io.StringIO()):
                self.assertEqual(checker.main(), 1)

    def test_main_accepts_matching_enum_and_matrix(self) -> None:
        plan = """## P0 2 变体门禁矩阵

| # | `ClientRequestV1` | 距离 | 维度 | 所有权 | 状态 | 结论 |
|---:|---|---|---|---|---|---|
| 1 | `Alpha` | — | — | — | — | — |
| 2 | `Beta` | — | — | — | — | — |

## 后续清单

| # | `ClientRequestV1` |
|---:|---|
| 99 | `Decoy` |
"""
        self.assertEqual(
            checker.parse_matrix_variants(plan),
            ([1, 2], ["Alpha", "Beta"]),
        )
        malformed = plan.replace(
            "| 2 | `Beta` | — | — | — | — | — |",
            "| x | `Gamma` | — | — | — | — | — |\n"
            "| 2 | `Beta` | — | — | — | — | — |",
        )
        with self.assertRaisesRegex(RuntimeError, "malformed C2S matrix row"):
            checker.parse_matrix_variants(malformed)
        with self.assertRaisesRegex(RuntimeError, "heading count 2 does not match 3 rows"):
            checker.parse_matrix_variants(
                plan.replace(
                    "\n## 后续清单",
                    "\n\n| 3 | `Gamma` | — | — | — | — | — |\n\n## 后续清单",
                )
            )
        with self.assertRaisesRegex(RuntimeError, "heading count 3 does not match 2 rows"):
            checker.parse_matrix_variants(plan.replace("P0 2", "P0 3"))

        with patch.object(
            checker, "enum_variants", return_value=["Alpha", "Beta"]
        ), patch.object(
            checker, "matrix_variants", return_value=([1, 2], ["Alpha", "Beta"])
        ):
            self.assertEqual(checker.main(), 0)


if __name__ == "__main__":
    unittest.main()
