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
        with self.assertRaisesRegex(
            RuntimeError, 'unsupported serde wire option|rename_all = "snake_case"'
        ):
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

    def test_rejects_commented_serde_contract_options(self) -> None:
        sources = [
            '#[serde(tag = "kind", rename_all = "camelCase", /* tag = "type", rename_all = "snake_case" */)]\npub enum ClientRequestV1 { Variant, }',
            '#[serde(tag = "type", /* rename_all = "snake_case" */ rename_all = "camelCase")]\npub enum ClientRequestV1 { Variant, }',
        ]
        for source in sources:
            with self.subTest(source=source), self.assertRaisesRegex(
                RuntimeError, "unsupported serde wire option|must use"
            ):
                checker.parse_enum_variants(source)

    def test_rejects_serde_wire_variant_attributes(self) -> None:
        for attribute in ("skip", "skip_deserializing", "skip_serializing", 'rename = "other"'):
            source = ENUM_PREFIX + f"pub enum ClientRequestV1 {{ #[serde({attribute})] Variant, }}"
            with self.subTest(attribute=attribute), self.assertRaisesRegex(
                RuntimeError, "variant-level serde"
            ):
                checker.parse_enum_variants(source)

    def test_accepts_multiline_serde_contract(self) -> None:
        source = """#[serde(
    deny_unknown_fields,
    tag = "type",
    rename_all = "snake_case",
)]
pub enum ClientRequestV1 { Variant, }
"""
        self.assertEqual(checker.parse_enum_variants(source), ["Variant"])

    def test_rejects_enum_level_serde_wire_options(self) -> None:
        for options in (
            'tag = "type", content = "payload", rename_all = "snake_case"',
            'tag = "type", rename_all = "snake_case", rename = "other"',
            'untagged, tag = "type", rename_all = "snake_case"',
        ):
            source = f"#[serde({options})]\npub enum ClientRequestV1 {{ Variant, }}"
            with self.subTest(options=options), self.assertRaisesRegex(
                RuntimeError, "unsupported serde wire option|must use"
            ):
                checker.parse_enum_variants(source)

    def test_rejects_multiple_enum_level_serde_attributes(self) -> None:
        sources = [
            """#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
#[serde(content = "payload")]
pub enum ClientRequestV1 { Variant, }
""",
            """#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum ClientRequestV1 { Variant, }
""",
        ]
        for source in sources:
            with self.subTest(source=source), self.assertRaisesRegex(
                RuntimeError, "unsupported serde wire option|rename_all"
            ):
                checker.parse_enum_variants(source)

    def test_accepts_whitespace_tolerant_enum_serde_attributes(self) -> None:
        for attribute in (
            '#[ serde(deny_unknown_fields, tag = "type", rename_all = "snake_case") ]',
            '#[serde (deny_unknown_fields, tag = "type", rename_all = "snake_case")]',
            '#[ serde ( deny_unknown_fields, tag = "type", rename_all = "snake_case" ) ]',
        ):
            with self.subTest(attribute=attribute):
                self.assertEqual(
                    checker.parse_enum_variants(
                        f'{attribute}\npub enum ClientRequestV1 {{ Variant, }}'
                    ),
                    ["Variant"],
                )

    def test_rejects_whitespace_tolerant_variant_serde_attributes(self) -> None:
        for attribute in (
            '#[ serde(rename = "other") ]',
            "#[serde (skip)]",
        ):
            source = (
                '#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]\n'
                f'pub enum ClientRequestV1 {{ {attribute} Variant, }}'
            )
            with self.subTest(attribute=attribute), self.assertRaisesRegex(
                RuntimeError, "variant-level serde"
            ):
                checker.parse_enum_variants(source)

    def test_requires_deny_unknown_fields_on_enum_contract(self) -> None:
        missing = '#[serde(tag = "type", rename_all = "snake_case")]\npub enum ClientRequestV1 { Variant, }'
        duplicate = '#[serde(deny_unknown_fields, deny_unknown_fields, tag = "type", rename_all = "snake_case")]\npub enum ClientRequestV1 { Variant, }'
        for source in (missing, duplicate):
            with self.subTest(source=source), self.assertRaisesRegex(
                RuntimeError, "deny_unknown_fields"
            ):
                checker.parse_enum_variants(source)

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
        with self.assertRaisesRegex(
            RuntimeError, 'unsupported serde wire option|tag = "type"'
        ):
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
            ), patch.object(
                checker, "generated_schema_variants", return_value=["alpha", "beta"]
            ), patch.object(
                checker, "KNOWN_TYPEBOX_GAPS", frozenset()
            ), redirect_stderr(io.StringIO()):
                self.assertEqual(checker.main(), 1)

    def test_main_accepts_matching_enum_and_matrix(self) -> None:
        plan = """## P0 2 变体门禁矩阵

| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |
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

        malformed_columns = {
            "too few row columns": plan.replace(
                "| 2 | `Beta` | — | — | — | — | — |",
                "| 2 | `Beta` | — | — | — | — |",
            ),
            "too many row columns": plan.replace(
                "| 2 | `Beta` | — | — | — | — | — |",
                "| 2 | `Beta` | — | — | — | — | — | extra |",
            ),
            "too few header columns": plan.replace(
                "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
                "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 |",
            ),
            "too many header columns": plan.replace(
                "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
                "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 | extra |",
            ),
        }
        for label, malformed_source in malformed_columns.items():
            with self.subTest(label=label), self.assertRaisesRegex(
                RuntimeError, r"malformed C2S matrix (header|row) at line \d+"
            ):
                checker.parse_matrix_variants(malformed_source)

        for header in (
            "| # | `ClientRequestV1` | 错误 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
            "| # | `ClientRequestV1` | 维度 | 距离 | 所有权 / participant | 状态前置 | P0 现状结论 |",
        ):
            with self.subTest(header=header), self.assertRaisesRegex(
                RuntimeError, r"malformed C2S matrix header at line \d+"
            ):
                checker.parse_matrix_variants(plan.replace(
                    "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
                    header,
                ))

        with self.assertRaisesRegex(RuntimeError, "heading count 2 does not match 3 rows"):
            checker.parse_matrix_variants(
                plan.replace(
                    "\n## 后续清单",
                    "\n\n| 3 | `Gamma` | — | — | — | — | — |\n\n## 后续清单",
                )
            )
        with self.assertRaisesRegex(RuntimeError, "heading count 3 does not match 2 rows"):
            checker.parse_matrix_variants(plan.replace("P0 2", "P0 3"))

        with self.assertRaisesRegex(RuntimeError, "unexpected C2S matrix table content before header"):
            checker.parse_matrix_variants(
                plan.replace(
                    "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
                    "| 999 | `StaleVariant` | — | — | — | — | — |\n"
                    "| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |",
                )
            )

        with patch.object(
            checker, "enum_variants", return_value=["Alpha", "Beta"]
        ), patch.object(
            checker, "matrix_variants", return_value=([1, 2], ["Alpha", "Beta"])
        ), patch.object(
            checker, "generated_schema_variants", return_value=["alpha", "beta"]
        ), patch.object(
            checker, "KNOWN_TYPEBOX_GAPS", frozenset()
        ):
            self.assertEqual(checker.main(), 0)

    def test_typebox_contract_rejects_unexpected_schema_drift(self) -> None:
        enum = ["Alpha", "Beta"]
        matrix = ["Alpha", "Beta"]
        with patch.object(checker, "KNOWN_TYPEBOX_GAPS", frozenset()):
            self.assertEqual(
                checker.typebox_contract_errors(enum, matrix, ["alpha", "beta"]),
                [],
            )
            self.assertEqual(
                checker.typebox_contract_errors(enum, matrix, ["alpha"]),
                ["Rust variants missing from TypeBox schema outside documented gaps: ['beta']"],
            )
            self.assertEqual(
                checker.typebox_contract_errors(enum, matrix, ["alpha", "beta", "gamma"]),
                [
                    "TypeBox schema variants absent from Rust enum: ['gamma']",
                    "TypeBox schema variants absent from Markdown matrix: ['gamma']",
                ],
            )

    def test_typebox_contract_exercises_documented_gap_baseline(self) -> None:
        enum = ["Alpha", "CoffinBreak"]
        matrix = ["Alpha", "CoffinBreak"]
        with patch.object(checker, "KNOWN_TYPEBOX_GAPS", frozenset({"coffin_break"})):
            self.assertEqual(
                checker.typebox_contract_errors(enum, matrix, ["alpha"]),
                [],
            )
            self.assertEqual(
                checker.typebox_contract_errors(enum, matrix, ["alpha", "coffin_break"]),
                ["documented TypeBox gaps are now present in schema: ['coffin_break']"],
            )
            self.assertEqual(
                checker.typebox_contract_errors(["Alpha"], ["Alpha"], ["alpha"]),
                ["TypeBox gap baseline names are not Rust variants: ['coffin_break']"],
            )

    def test_serde_snake_case_preserves_acronym_boundaries(self) -> None:
        self.assertEqual(checker.rust_variant_to_wire("HTTPStatus"), "h_t_t_p_status")

        plan = """## P0 2 变体门禁矩阵

| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |
|---:|---|---|---|---|---|---|
| 1 | `Alpha` | — | — | — | — | — |
| 2 | `Beta` | — | — | — | — | — |

## 后续清单

stale notes

## P0 2 变体门禁矩阵

| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |
|---:|---|---|---|---|---|---|
| 1 | `Beta` | — | — | — | — | — |
| 2 | `Alpha` | — | — | — | — | — |
"""
        with self.assertRaisesRegex(RuntimeError, "duplicate P0 matrix sections"):
            checker.parse_matrix_variants(plan)


if __name__ == "__main__":
    unittest.main()
