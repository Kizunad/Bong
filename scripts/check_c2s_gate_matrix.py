#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENUM_PATH = ROOT / "server/src/schema/client_request.rs"
PLAN_PATH = ROOT / "docs/plan-refactor-c2s-gate-v1.md"
ENUM_DECL_RE = re.compile(r"(?m)^[ \t]*pub\s+enum\s+ClientRequestV1\s*\{")
SERDE_ATTR_RE = re.compile(r"(?m)^[ \t]*#\[serde\(([^]]*)\)\][ \t]*$")
RUST_NON_CODE_RE = re.compile(
    r'//[^\n]*|(?s:/\*.*?\*/)|(?:b)?r(?P<hashes>#+)"(?s:.*?)"(?P=hashes)|(?:b)?r"(?s:.*?)"|"(?:\\.|[^"\\])*"'
)
MATRIX_RE = re.compile(r"^\|\s*(\d+)\s*\|\s*`([^`]+)`\s*\|")
VARIANT_DECL_RE = re.compile(r"^([A-Z][A-Za-z0-9_]*)\s*(.*)$")


def _mask_rust_non_code(source: str) -> str:
    return RUST_NON_CODE_RE.sub(
        lambda match: "".join("\n" if char == "\n" else " " for char in match.group()),
        source,
    )


def _without_line_comment(line: str) -> str:
    return line.split("//", 1)[0]


def _leading_attributes(code: str) -> tuple[list[str], str]:
    attributes: list[str] = []
    while code.startswith("#["):
        bracket_depth = 0
        in_string = False
        escaped = False
        end = None
        for index, char in enumerate(code):
            if in_string:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    in_string = False
                continue
            if char == '"':
                in_string = True
            elif char == "[":
                bracket_depth += 1
            elif char == "]":
                bracket_depth -= 1
                if bracket_depth == 0:
                    end = index + 1
                    break
        if end is None:
            raise RuntimeError(f"unterminated ClientRequestV1 attribute: {code!r}")
        attributes.append(code[:end])
        code = code[end:].lstrip()
    if code.startswith("#"):
        raise RuntimeError(f"unsupported ClientRequestV1 attribute syntax: {code!r}")
    return attributes, code


def parse_enum_variants(source: str) -> list[str]:
    masked = _mask_rust_non_code(source)
    declaration = ENUM_DECL_RE.search(masked)
    if not declaration:
        raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")
    serde = None
    for match in reversed(list(SERDE_ATTR_RE.finditer(source, 0, declaration.start()))):
        if source[match.end() : declaration.start()].strip():
            break
        serde = match.group(1)
        break
    if serde is None or not re.search(r'\btag\s*=\s*"type"', serde):
        raise RuntimeError("ClientRequestV1 must use serde tag = \"type\"")
    if not re.search(r'\brename_all\s*=\s*"snake_case"', serde):
        raise RuntimeError("ClientRequestV1 must use serde rename_all = \"snake_case\"")

    lines = source.splitlines()
    masked_lines = masked.splitlines()
    variants: list[str] = []
    inside = False
    depth = 0
    tuple_depth = 0
    pending_attributes: list[str] = []

    for line, masked_line in zip(lines, masked_lines):
        if not inside:
            if masked_line.strip() == "pub enum ClientRequestV1 {":
                inside = True
                depth = 1
            continue

        code = _without_line_comment(line).strip()
        structural_code = masked_line.strip()
        if depth == 1 and tuple_depth:
            tuple_depth += structural_code.count("(") - structural_code.count(")")
            if tuple_depth < 0:
                raise RuntimeError(f"unbalanced tuple variant syntax: {line!r}")
            continue
        if depth == 1 and structural_code == "}":
            depth = 0
            break
        if depth == 1 and structural_code:
            if code == ",":
                continue
            attributes, code = _leading_attributes(code)
            pending_attributes.extend(attributes)
            if not code:
                continue
            if any("serde" in attribute and "rename" in attribute for attribute in pending_attributes):
                raise RuntimeError("ClientRequestV1 variant-level serde rename is unsupported")
            pending_attributes.clear()
            match = VARIANT_DECL_RE.match(code)
            if not match:
                raise RuntimeError(f"unsupported ClientRequestV1 syntax: {line!r}")
            suffix = match.group(2).strip()
            if not suffix or suffix[0] not in "{(,":
                raise RuntimeError(f"unsupported ClientRequestV1 variant syntax: {line!r}")
            variants.append(match.group(1))
            if suffix[0] == "(":
                tuple_depth = suffix.count("(") - suffix.count(")")
                if tuple_depth < 0:
                    raise RuntimeError(f"unbalanced tuple variant syntax: {line!r}")

        depth += structural_code.count("{") - structural_code.count("}")
        if depth == 0:
            break

    if not inside or depth != 0 or not variants:
        raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")
    return variants


def enum_variants() -> list[str]:
    return parse_enum_variants(ENUM_PATH.read_text(encoding="utf-8"))


def matrix_variants() -> tuple[list[int], list[str]]:
    rows: list[tuple[int, str]] = []
    for line in PLAN_PATH.read_text(encoding="utf-8").splitlines():
        if match := MATRIX_RE.match(line):
            rows.append((int(match.group(1)), match.group(2)))
    if not rows:
        raise RuntimeError(f"cannot parse C2S matrix from {PLAN_PATH}")
    return [number for number, _ in rows], [variant for _, variant in rows]


def duplicates(values: list[str]) -> list[str]:
    return sorted(value for value, count in Counter(values).items() if count > 1)


def first_order_mismatch(left: list[str], right: list[str]) -> tuple[int, str, str] | None:
    for index in range(max(len(left), len(right))):
        left_value = left[index] if index < len(left) else "<missing>"
        right_value = right[index] if index < len(right) else "<missing>"
        if left_value != right_value:
            return index, left_value, right_value
    return None


def main() -> int:
    errors: list[str] = []
    try:
        enum = enum_variants()
        numbers, matrix = matrix_variants()
    except RuntimeError as error:
        print(f"C2S gate matrix check failed:\n- {error}", file=sys.stderr)
        return 1

    expected_numbers = list(range(1, len(matrix) + 1))
    if numbers != expected_numbers:
        errors.append(f"matrix numbering is not contiguous: {numbers}")

    for label, values in (("enum", enum), ("matrix", matrix)):
        duplicate_values = duplicates(values)
        if duplicate_values:
            errors.append(f"duplicate {label} variants: {duplicate_values}")

    enum_matrix_missing = [variant for variant in enum if variant not in matrix]
    matrix_enum_extra = [variant for variant in matrix if variant not in enum]
    if enum_matrix_missing:
        errors.append(f"missing matrix variants: {enum_matrix_missing}")
    if matrix_enum_extra:
        errors.append(f"extra matrix variants: {matrix_enum_extra}")
    if set(matrix) != set(enum):
        errors.append("matrix and Rust enum variant sets differ")

    mismatch = first_order_mismatch(enum, matrix)
    if mismatch:
        errors.append(
            "first enum/matrix order mismatch at row "
            f"{mismatch[0] + 1}: enum={mismatch[1]} matrix={mismatch[2]}"
        )

    if errors:
        print("C2S gate matrix check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"C2S gate matrix matches all {len(enum)} Rust ClientRequestV1 variants")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
