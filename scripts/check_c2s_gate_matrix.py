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
ATTRIBUTE_LINE_RE = re.compile(r"(?m)^[ \t]*#\[[^\n]*\][ \t]*$")
MATRIX_SECTION_RE = re.compile(r"^## P0 (\d+) 变体门禁矩阵\s*$")
MATRIX_HEADER_RE = re.compile(
    r"^\|\s*#\s*\|\s*`ClientRequestV1`\s*\|(?:[^|\n]*\|){5}\s*$"
)
MATRIX_DIVIDER_RE = re.compile(r"^\|(?:\s*:?-+:?\s*\|){7}$")
MATRIX_RE = re.compile(
    r"^\|\s*(\d+)\s*\|\s*`([^`]+)`\s*\|(?:[^|\n]*\|){5}\s*$"
)
VARIANT_DECL_RE = re.compile(r"^([A-Z][A-Za-z0-9_]*)\s*(.*)$", re.DOTALL)


def _blank_non_newlines(masked: list[str], start: int, end: int) -> None:
    for index in range(start, end):
        if masked[index] != "\n":
            masked[index] = " "


def _mask_rust_non_code(source: str, *, mask_strings: bool = True) -> str:
    masked = list(source)
    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end == -1 else end
            _blank_non_newlines(masked, index, end)
            index = end
            continue
        if source.startswith("/*", index):
            start = index
            depth = 1
            index += 2
            while index < len(source) and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            _blank_non_newlines(masked, start, index)
            continue
        raw_prefix = 2 if source.startswith("br", index) else 1 if source.startswith("r", index) else 0
        if raw_prefix:
            quote_index = index + raw_prefix
            while quote_index < len(source) and source[quote_index] == "#":
                quote_index += 1
            if quote_index < len(source) and source[quote_index] == '"':
                delimiter = '"' + source[index + raw_prefix : quote_index]
                end = source.find(delimiter, quote_index + 1)
                end = len(source) if end == -1 else end + len(delimiter)
                if mask_strings:
                    _blank_non_newlines(masked, index, end)
                index = end
                continue
        if source[index] == '"':
            end = index + 1
            while end < len(source):
                if source[end] == "\\":
                    end += 2
                elif source[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            if mask_strings:
                _blank_non_newlines(masked, index, end)
            index = end
            continue
        index += 1
    return "".join(masked)


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


def _wrapped_suffix(suffix: str, opening: str, closing: str) -> bool:
    if not suffix.startswith(opening):
        return False
    depth = 0
    for index, char in enumerate(suffix):
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index == len(suffix) - 1
            if depth < 0:
                return False
    return False


def _enum_body(masked: str, declaration_end: int) -> tuple[int, int]:
    depth = 1
    for index in range(declaration_end, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return declaration_end, index
    raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")


def _variant_fragments(body: str) -> list[str]:
    fragments: list[str] = []
    start = 0
    paren_depth = brace_depth = bracket_depth = 0
    for index, char in enumerate(body):
        if char == "(":
            paren_depth += 1
        elif char == ")":
            paren_depth -= 1
        elif char == "{":
            brace_depth += 1
        elif char == "}":
            brace_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
        elif char == "," and paren_depth == brace_depth == bracket_depth == 0:
            fragments.append(body[start:index])
            start = index + 1
        if min(paren_depth, brace_depth, bracket_depth) < 0:
            raise RuntimeError("unbalanced ClientRequestV1 variant syntax")
    if paren_depth or brace_depth or bracket_depth:
        raise RuntimeError("unbalanced ClientRequestV1 variant syntax")
    fragments.append(body[start:])
    return fragments


def parse_enum_variants(source: str) -> list[str]:
    masked = _mask_rust_non_code(source)
    declaration = ENUM_DECL_RE.search(masked)
    if not declaration:
        raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")
    serde = None
    attribute_end = declaration.start()
    for match in reversed(list(ATTRIBUTE_LINE_RE.finditer(masked, 0, declaration.start()))):
        if masked[match.end() : attribute_end].strip():
            break
        attribute = source[match.start() : match.end()].strip()
        if attribute.startswith("#[serde(") and attribute.endswith(")]"):
            serde = _mask_rust_non_code(attribute, mask_strings=False)[len("#[serde(") : -2]
            break
        attribute_end = match.start()
    tag_options = re.findall(r'\btag\s*=\s*"([^"]*)"', serde or "")
    rename_options = re.findall(r'\brename_all\s*=\s*"([^"]*)"', serde or "")
    if len(tag_options) != 1 or tag_options[0] != "type":
        raise RuntimeError("ClientRequestV1 must use serde tag = \"type\"")
    if len(rename_options) != 1 or rename_options[0] != "snake_case":
        raise RuntimeError("ClientRequestV1 must use serde rename_all = \"snake_case\"")
    if re.search(r"\buntagged\b", serde or ""):
        raise RuntimeError("ClientRequestV1 serde untagged representation is unsupported")

    body_start, body_end = _enum_body(masked, declaration.end())
    variants: list[str] = []
    for fragment in _variant_fragments(masked[body_start:body_end]):
        code = fragment.strip()
        if not code:
            continue
        attributes, code = _leading_attributes(code)
        serde_attributes = [attribute for attribute in attributes if attribute.startswith("#[serde(")]
        if serde_attributes:
            if any("rename" in attribute for attribute in serde_attributes):
                raise RuntimeError("ClientRequestV1 variant-level serde rename is unsupported")
            raise RuntimeError("ClientRequestV1 variant-level serde wire attribute is unsupported")
        match = VARIANT_DECL_RE.fullmatch(code)
        if not match:
            raise RuntimeError(f"unsupported ClientRequestV1 syntax: {code!r}")
        suffix = match.group(2).strip()
        if suffix and not (
            _wrapped_suffix(suffix, "(", ")") or _wrapped_suffix(suffix, "{", "}")
        ):
            raise RuntimeError(f"unsupported ClientRequestV1 variant syntax: {code!r}")
        variants.append(match.group(1))

    if not variants:
        raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")
    return variants


def enum_variants() -> list[str]:
    return parse_enum_variants(ENUM_PATH.read_text(encoding="utf-8"))


def parse_matrix_variants(source: str) -> tuple[list[int], list[str]]:
    rows: list[tuple[int, str]] = []
    expected_count = None
    header_seen = False
    divider_seen = False
    table_ended = False
    for line_number, line in enumerate(source.splitlines(), start=1):
        if expected_count is None:
            if section := MATRIX_SECTION_RE.fullmatch(line):
                expected_count = int(section.group(1))
            continue
        if line.startswith("## "):
            break
        if not header_seen:
            if MATRIX_HEADER_RE.fullmatch(line):
                header_seen = True
            continue
        if not divider_seen:
            if MATRIX_DIVIDER_RE.fullmatch(line):
                divider_seen = True
                continue
            if line.strip():
                raise RuntimeError(f"malformed C2S matrix divider at line {line_number}: {line!r}")
            continue
        if not line.strip():
            continue
        if table_ended:
            if line.startswith("|"):
                raise RuntimeError(
                    f"unexpected C2S matrix content at line {line_number}: {line!r}"
                )
            continue
        if not line.startswith("|"):
            table_ended = True
            continue
        match = MATRIX_RE.fullmatch(line)
        if not match:
            raise RuntimeError(f"malformed C2S matrix row at line {line_number}: {line!r}")
        rows.append((int(match.group(1)), match.group(2)))
    if not rows:
        raise RuntimeError(f"cannot parse C2S matrix from {PLAN_PATH}")
    if expected_count != len(rows):
        raise RuntimeError(
            f"C2S matrix heading count {expected_count} does not match {len(rows)} rows"
        )
    return [number for number, _ in rows], [variant for _, variant in rows]


def matrix_variants() -> tuple[list[int], list[str]]:
    return parse_matrix_variants(PLAN_PATH.read_text(encoding="utf-8"))


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
