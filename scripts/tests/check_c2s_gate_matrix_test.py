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
    UnitVariant,
    TupleVariant(u8),
}
"""
        self.assertEqual(
            checker.parse_enum_variants(source),
            ["StructVariant", "UnitVariant", "TupleVariant"],
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

    def test_fails_closed_on_unknown_top_level_syntax(self) -> None:
        source = ENUM_PREFIX + """pub enum ClientRequestV1 {
    #[cfg(test)]
    Unsupported = 1,
}
"""
        with self.assertRaisesRegex(RuntimeError, "unsupported ClientRequestV1 variant syntax"):
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
        with patch.object(
            checker, "enum_variants", return_value=["Alpha", "Beta"]
        ), patch.object(
            checker, "matrix_variants", return_value=([1, 2], ["Alpha", "Beta"])
        ):
            self.assertEqual(checker.main(), 0)


if __name__ == "__main__":
    unittest.main()
