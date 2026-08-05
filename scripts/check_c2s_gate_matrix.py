#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENUM_PATH = ROOT / "server/src/schema/client_request.rs"
PLAN_PATH = ROOT / "docs/plan-refactor-c2s-gate-v1.md"
GENERATED_SCHEMA_PATH = ROOT / "agent/packages/schema/generated/client-request-v1.json"
ENUM_DECL_RE = re.compile(r"(?m)^[ \t]*pub\s+enum\s+ClientRequestV1\s*\{")
MATRIX_SECTION_RE = re.compile(r"^## P0 (\d+) 变体门禁矩阵\s*$")
MATRIX_HEADER_RE = re.compile(
    r"^\|\s*#\s*\|\s*`ClientRequestV1`\s*\|\s*距离\s*\|\s*维度\s*\|\s*所有权 / participant\s*\|\s*状态前置\s*\|\s*P0 现状结论\s*\|\s*$"
)
MATRIX_DIVIDER_RE = re.compile(r"^\|(?:\s*:?-+:?\s*\|){7}$")
MATRIX_RE = re.compile(
    r"^\|\s*(\d+)\s*\|\s*`([^`]+)`\s*\|(?:[^|\n]*\|){5}\s*$"
)
VARIANT_DECL_RE = re.compile(r"^([A-Z][A-Za-z0-9_]*)\s*(.*)$", re.DOTALL)
KNOWN_TYPEBOX_GAPS = frozenset(
    {
        "alchemy_learn_recipe_fragment",
        "coffin_break",
        "coffin_menu_reclaim",
        "qi_scatter_bead_use",
        "jiemai",
        "supply_coffin_open",
        "container_open",
        "workbench_open",
        "external_container_move",
        "external_container_close",
        "lingtian_start_till",
        "lingtian_start_renew",
        "lingtian_start_planting",
        "lingtian_start_harvest",
        "lingtian_start_drain_qi",
        "craft_start",
        "craft_cancel",
        "give_dan_to_elder",
    }
)


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
    while match := re.match(r"^#\s*\[", code):
        bracket_start = match.end() - 1
        bracket_depth = 0
        in_string = False
        escaped = False
        end = None
        for index in range(bracket_start, len(code)):
            char = code[index]
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


def _enum_attributes(masked: str, source: str, declaration_start: int) -> list[str]:
    attributes: list[tuple[int, int, str]] = []
    for match in re.finditer(r"#\s*\[", masked[:declaration_start]):
        depth = 0
        end = None
        for index in range(match.start(), declaration_start):
            char = masked[index]
            if char == "[":
                depth += 1
            elif char == "]":
                depth -= 1
                if depth == 0:
                    end = index + 1
                    break
        if end is not None:
            attributes.append((match.start(), end, source[match.start() : end].strip()))

    selected: list[str] = []
    cursor = declaration_start
    for start, end, attribute in reversed(attributes):
        if masked[end:cursor].strip():
            break
        selected.append(attribute)
        cursor = start
    return list(reversed(selected))


def _serde_options(serde: str) -> list[str]:
    options: list[str] = []
    start = 0
    in_string = False
    escaped = False
    for index, char in enumerate(serde):
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
        elif char == '"':
            in_string = True
        elif char == ",":
            options.append(serde[start:index].strip())
            start = index + 1
    options.append(serde[start:].strip())
    return [option for option in options if option]


def _serde_attribute_body(attribute: str) -> str | None:
    attribute = attribute.strip()
    match = re.match(r"^#\s*\[\s*serde\b", attribute)
    if not match:
        return None
    opening = attribute.find("(", match.end())
    closing_bracket = attribute.rfind("]")
    if opening == -1 or closing_bracket <= opening:
        raise RuntimeError("unsupported ClientRequestV1 serde attribute syntax")
    if attribute[match.end() : opening].strip():
        raise RuntimeError("unsupported ClientRequestV1 serde attribute syntax")
    suffix = attribute[opening + 1 : closing_bracket].rstrip()
    if not suffix.endswith(")") or attribute[closing_bracket + 1 :].strip():
        raise RuntimeError("unsupported ClientRequestV1 serde attribute syntax")
    return suffix[:-1]


def _normalize_serde_option(option: str) -> str:
    option = re.sub(r"\s+", " ", option.strip())
    return re.sub(r"\s*=\s*", " = ", option)


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
    enum_attributes = _enum_attributes(masked, source, declaration.start())
    serde_options: list[str] = []
    for attribute in enum_attributes:
        body = _serde_attribute_body(attribute)
        if body is None:
            continue
        masked_body = _mask_rust_non_code(body, mask_strings=False)
        serde_options.extend(
            _normalize_serde_option(option)
            for option in _serde_options(masked_body)
            if option
        )

    allowed_options = {
        'tag = "type"',
        'rename_all = "snake_case"',
        "deny_unknown_fields",
    }
    if any(option not in allowed_options for option in serde_options):
        raise RuntimeError("ClientRequestV1 has unsupported serde wire option")
    if serde_options.count("deny_unknown_fields") != 1:
        raise RuntimeError('ClientRequestV1 must use serde deny_unknown_fields')
    tag_options = [option for option in serde_options if option.startswith("tag = ")]
    rename_options = [option for option in serde_options if option.startswith("rename_all = ")]
    if tag_options != ['tag = "type"']:
        raise RuntimeError("ClientRequestV1 must use serde tag = \"type\"")
    if rename_options != ['rename_all = "snake_case"']:
        raise RuntimeError("ClientRequestV1 must use serde rename_all = \"snake_case\"")

    body_start, body_end = _enum_body(masked, declaration.end())
    variants: list[str] = []
    for fragment in _variant_fragments(masked[body_start:body_end]):
        code = fragment.strip()
        if not code:
            continue
        attributes, code = _leading_attributes(code)
        serde_attributes = [
            attribute
            for attribute in attributes
            if _serde_attribute_body(attribute) is not None
        ]
        if serde_attributes:
            if any(
                any(
                    _normalize_serde_option(option).startswith("rename")
                    for option in _serde_options(
                        _mask_rust_non_code(
                            _serde_attribute_body(attribute) or "",
                            mask_strings=False,
                        )
                    )
                )
                for attribute in serde_attributes
            ):
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

    # The C2S gate matrix is a singular contract: exactly one `## P0 N
    # 变体门禁矩阵` section may exist in the plan. A second authoritative-
    # looking matrix (stale, reordered, or with extra variants) must fail
    # closed instead of being silently ignored after the first section.
    matrix_sections = [
        line
        for line in source.splitlines()
        if MATRIX_SECTION_RE.fullmatch(line)
    ]
    if len(matrix_sections) > 1:
        raise RuntimeError(
            "C2S plan contains duplicate P0 matrix sections: "
            f"{len(matrix_sections)} headings ({matrix_sections[0]!r}, "
            f"{matrix_sections[1]!r}, ...)"
        )

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
            elif line.strip() and line.lstrip().startswith("|"):
                if "ClientRequestV1" in line:
                    raise RuntimeError(
                        f"malformed C2S matrix header at line {line_number}: {line!r}"
                    )
                raise RuntimeError(
                    f"unexpected C2S matrix table content before header at line {line_number}: {line!r}"
                )
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


def rust_variant_to_wire(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def generated_schema_variants() -> list[str]:
    try:
        document = json.loads(GENERATED_SCHEMA_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError(f"cannot parse generated ClientRequestV1 schema: {error}") from error

    members = document.get("anyOf") if isinstance(document, dict) else None
    if not isinstance(members, list) or not members:
        raise RuntimeError("generated ClientRequestV1 schema must contain a non-empty anyOf list")

    variants: list[str] = []
    for index, member in enumerate(members, start=1):
        try:
            variant = member["properties"]["type"]["const"]
        except (KeyError, TypeError):
            raise RuntimeError(
                f"generated ClientRequestV1 schema member {index} lacks a type const"
            ) from None
        if not isinstance(variant, str) or not variant:
            raise RuntimeError(
                f"generated ClientRequestV1 schema member {index} has an invalid type const"
            )
        variants.append(variant)
    return variants


def typebox_contract_errors(enum: list[str], matrix: list[str], schema: list[str]) -> list[str]:
    rust_wire = [rust_variant_to_wire(variant) for variant in enum]
    matrix_wire = [rust_variant_to_wire(variant) for variant in matrix]
    rust_set = set(rust_wire)
    matrix_set = set(matrix_wire)
    schema_set = set(schema)
    errors: list[str] = []

    unknown_gaps = sorted(KNOWN_TYPEBOX_GAPS - rust_set)
    if unknown_gaps:
        errors.append(f"TypeBox gap baseline names are not Rust variants: {unknown_gaps}")
    if duplicates(schema):
        errors.append(f"duplicate TypeBox schema variants: {duplicates(schema)}")

    stale_gaps = sorted(KNOWN_TYPEBOX_GAPS & schema_set)
    if stale_gaps:
        errors.append(f"documented TypeBox gaps are now present in schema: {stale_gaps}")

    extra_schema = sorted(schema_set - rust_set)
    if extra_schema:
        errors.append(f"TypeBox schema variants absent from Rust enum: {extra_schema}")
    extra_matrix = sorted(schema_set - matrix_set)
    if extra_matrix:
        errors.append(f"TypeBox schema variants absent from Markdown matrix: {extra_matrix}")

    undocumented_missing = sorted((rust_set - schema_set) - KNOWN_TYPEBOX_GAPS)
    if undocumented_missing:
        errors.append(f"Rust variants missing from TypeBox schema outside documented gaps: {undocumented_missing}")
    return errors


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
        schema = generated_schema_variants()
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

    errors.extend(typebox_contract_errors(enum, matrix, schema))

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
