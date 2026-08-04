#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENUM_PATH = ROOT / "server/src/schema/client_request.rs"
PLAN_PATH = ROOT / "docs/plan-refactor-c2s-gate-v1.md"
ENUM_DECL_RE = re.compile(
    r"(?m)^[ \t]*pub\s+enum\s+ClientRequestV1\s*\{"
)
ATTRIBUTE_LINE_RE = re.compile(r"(?m)^[ \t]*#\[[^\n]*\][ \t]*(?:\n|$)")
MATRIX_RE = re.compile(r"^\|\s*(\d+)\s*\|\s*`([^`]+)`\s*\|")
VARIANT_DECL_RE = re.compile(r"^([A-Z][A-Za-z0-9_]*)\s*(.*)$")


def _mask_rust_comments_and_strings(source: str) -> str:
    masked = list(source)
    length = len(source)

    def blank(start: int, end: int) -> None:
        for index in range(start, end):
            if source[index] != "\n":
                masked[index] = " "

    index = 0
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end == -1 else end
            blank(index, end)
            index = end
            continue
        if source.startswith("/*", index):
            end = index + 2
            comment_depth = 1
            while end < length and comment_depth:
                if source.startswith("/*", end):
                    comment_depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    comment_depth -= 1
                    end += 2
                else:
                    end += 1
            blank(index, end)
            index = end
            continue

        raw = re.match(r"(?:b)?r(#+)\"", source[index:]) or re.match(
            r"(?:b)?r\"", source[index:]
        )
        if raw:
            hashes = raw.group(1) if raw.lastindex else ""
            delimiter = '"' + hashes
            content_start = index + raw.end()
            closing = source.find(delimiter, content_start)
            end = length if closing == -1 else closing + len(delimiter)
            blank(index, end)
            index = end
            continue

        if source[index] == "'" and not re.match(r"'(?:\\.|[^'\\\n])'", source[index:]):
            index += 1
            continue
        if source[index] in {'"', "'"}:
            quote = source[index]
            end = index + 1
            while end < length:
                if source[end] == "\\":
                    end += 2
                elif source[end] == quote:
                    end += 1
                    break
                else:
                    end += 1
            blank(index, end)
            index = end
            continue
        index += 1

    return "".join(masked)


def _enum_declaration(source: str) -> tuple[str, re.Match[str]]:
    masked = _mask_rust_comments_and_strings(source)
    declaration = ENUM_DECL_RE.search(masked)
    if not declaration:
        raise RuntimeError(f"cannot parse ClientRequestV1 from {ENUM_PATH}")
    return masked, declaration


def _enum_serde_body(source: str, masked: str, declaration: re.Match[str]) -> str:
    attributes: list[str] = []
    cursor = declaration.start()
    for match in reversed(list(ATTRIBUTE_LINE_RE.finditer(masked, 0, cursor))):
        if masked[match.end() : cursor].strip():
            break
        attributes.insert(0, source[match.start() : match.end()])
        cursor = match.start()
    for attribute in attributes:
        if serde := re.search(r"#\[serde\(([^]]*)\)\]", attribute):
            return serde.group(1)
    raise RuntimeError("ClientRequestV1 must declare a serde attribute")


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
    masked, declaration = _enum_declaration(source)
    serde = _enum_serde_body(source, masked, declaration)
    if not re.search(r'\btag\s*=\s*"type"', serde):
        raise RuntimeError("ClientRequestV1 must use serde tag = \"type\"")
    if not re.search(r'\brename_all\s*=\s*"snake_case"', serde):
        raise RuntimeError("ClientRequestV1 must use serde rename_all = \"snake_case\"")

    lines = source.splitlines()
    masked_lines = masked.splitlines()
    declaration_line = masked[: declaration.start()].count("\n")
    variants: list[str] = []
    inside = False
    depth = 0
    tuple_depth = 0
    pending_attributes: list[str] = []

    for line_number, (line, masked_line) in enumerate(zip(lines, masked_lines)):
        if not inside:
            if line_number == declaration_line:
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
