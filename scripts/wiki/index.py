#!/usr/bin/env python3
"""生成 Bong 代码 wiki 的跨端搜索入口。

三种文档工具的搜索索引格式并不稳定，因此入口优先读取原生索引，
再从生成的 HTML 标题建立保底索引。这样 rustdoc 升级或某一端暂时
缺失时，离线入口仍能工作，并且会明确标出缺失端。
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
from dataclasses import asdict, dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Any, Iterable


@dataclass(frozen=True)
class Symbol:
    """统一搜索结果的一条符号记录。"""

    endpoint: str
    name: str
    qualified: str
    kind: str
    url: str


class _TitleParser(HTMLParser):
    """提取单个文档页面的标题和首个标题元素。"""

    def __init__(self) -> None:
        super().__init__()
        self.title_parts: list[str] = []
        self.heading_parts: list[str] = []
        self._in_title = False
        self._in_heading = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self._in_title = tag == "title"
        self._in_heading = tag in {"h1", "h2"} and not self.heading_parts

    def handle_endtag(self, tag: str) -> None:
        if tag == "title":
            self._in_title = False
        if tag in {"h1", "h2"}:
            self._in_heading = False

    def handle_data(self, data: str) -> None:
        if self._in_title:
            self.title_parts.append(data)
        if self._in_heading:
            self.heading_parts.append(data)

    def title(self) -> str:
        return " ".join("".join(self.title_parts).split())

    def heading(self) -> str:
        return " ".join("".join(self.heading_parts).split())


def _assignment_payload(text: str, names: Iterable[str]) -> Any:
    """读取 JavaScript 赋值右侧的首个 JSON 值，失败时返回空列表。"""

    name_pattern = "|".join(re.escape(name) for name in names)
    match = re.search(rf"(?:var\s+)?(?:{name_pattern})\s*=\s*", text)
    if not match:
        return []
    start = match.end()
    while start < len(text) and text[start].isspace():
        start += 1
    try:
        return json.JSONDecoder().raw_decode(text[start:])[0]
    except json.JSONDecodeError:
        return []


def _entry_list(payload: Any) -> list[dict[str, Any]]:
    """把工具可能使用的数组或字典包装拆成条目列表。"""

    if isinstance(payload, list):
        return [entry for entry in payload if isinstance(entry, dict)]
    if isinstance(payload, dict):
        for key in ("doc", "items", "entries", "index"):
            if isinstance(payload.get(key), list):
                return [entry for entry in payload[key] if isinstance(entry, dict)]
    return []


def parse_rust_search_index(text: str) -> list[dict[str, str]]:
    """解析旧版 rustdoc 的 ``searchIndex``，新版压缩索引由 HTML 扫描兜底。"""

    payload = _assignment_payload(text, ("searchIndex", "SEARCH_INDEX"))
    result: list[dict[str, str]] = []
    for entry in _entry_list(payload):
        name = str(entry.get("n") or entry.get("name") or entry.get("t") or "").strip()
        url = str(entry.get("p") or entry.get("url") or "").strip()
        if name and url:
            result.append(
                {
                    "name": name,
                    "qualified": str(entry.get("q") or name),
                    "kind": str(entry.get("t") or "symbol"),
                    "url": url,
                }
            )
    return result


def parse_javadoc_search_index(text: str) -> list[dict[str, str]]:
    """解析 Javadoc 的类型或成员搜索索引，并还原离线页面链接。"""

    payload = _assignment_payload(
        text, ("typeSearchIndex", "memberSearchIndex", "packageSearchIndex")
    )
    result: list[dict[str, str]] = []
    for entry in _entry_list(payload):
        label = str(entry.get("l") or entry.get("m") or entry.get("name") or "").strip()
        url = str(entry.get("url") or "").strip()
        package = str(entry.get("p") or "").strip()
        owner = str(entry.get("c") or "").strip()
        if not url and package and owner:
            url = f"{package.replace('.', '/')}/{owner}.html"
            anchor = str(entry.get("u") or label).strip()
            if anchor:
                url = f"{url}#{anchor}"
        elif not url and package and label:
            url = f"{package.replace('.', '/')}/{label}.html"
        if label and url:
            name = label if owner else label.rsplit(".", 1)[-1]
            qualified = str(entry.get("q") or "").strip()
            if not qualified and package:
                qualified = f"{package}.{owner + '.' if owner else ''}{label}"
            result.append(
                {
                    "name": name,
                    "qualified": qualified or label,
                    "kind": "member" if owner else "type",
                    "url": url,
                }
            )
    return result


def parse_typedoc_search_index(text: str) -> list[dict[str, str]]:
    """解析 TypeDoc 的 ``searchData`` 数组。"""

    payload = _assignment_payload(text, ("searchData", "searchIndex"))
    result: list[dict[str, str]] = []
    for entry in _entry_list(payload):
        name = str(entry.get("name") or entry.get("label") or "").strip()
        url = str(entry.get("url") or entry.get("href") or "").strip()
        if name and url:
            result.append(
                {
                    "name": name,
                    "qualified": str(entry.get("qualifiedName") or name),
                    "kind": str(entry.get("kind") or "symbol"),
                    "url": url,
                }
            )
    return result


def _normalise_title(title: str, fallback: str) -> str:
    """把三种文档器的页面标题归一为可读符号名。"""

    value = title.strip()
    if value.endswith(" - Rust"):
        value = value[:-7].strip()
    for prefix in ("Class ", "Interface ", "Enum ", "Record ", "Namespace ", "Module "):
        if value.startswith(prefix):
            value = value[len(prefix) :].strip()
    return value or fallback


def _rust_symbol(title: str) -> tuple[str, str] | None:
    """从 rustdoc 的标题中还原符号名和完整路径。"""

    value = _normalise_title(title, "")
    match = re.fullmatch(r"(.+?)\s+in\s+(.+)", value)
    if not match:
        return None
    name = match.group(1).strip()
    module = match.group(2).strip()
    if not name or not module:
        return None
    return name.split("::")[-1], f"{module}::{name}"


def _html_symbol(endpoint: str, root: Path, path: Path) -> Symbol | None:
    """从一个 HTML 文档页提取符号，供原生索引缺失时使用。"""

    if path.name in {"index.html", "help.html", "settings.html"}:
        return None
    parser = _TitleParser()
    try:
        parser.feed(path.read_text(encoding="utf-8", errors="replace"))
    except OSError:
        return None
    relative = path.relative_to(root).as_posix()
    fallback = path.stem.replace(".", " ")
    title = parser.heading() or parser.title()
    name = _normalise_title(title, fallback)
    qualified = name
    if endpoint == "server":
        rust_symbol = _rust_symbol(parser.title())
        if rust_symbol:
            name, qualified = rust_symbol
    if not name or name.lower() in {"bong server", "bong client", "bong"}:
        return None
    path_kind = re.search(r"/(struct|enum|trait|fn|type|constant|macro|module)\.", f"/{relative}")
    kind = path_kind.group(1) if path_kind else "page"
    return Symbol(endpoint, name.split("::")[-1], qualified, kind, f"{endpoint}/{relative}")


def _scan_html(endpoint: str, root: Path) -> list[Symbol]:
    """扫描生成目录，建立所有可离线打开的页面索引。"""

    if not root.is_dir():
        return []
    symbols: list[Symbol] = []
    for path in sorted(root.rglob("*.html")):
        symbol = _html_symbol(endpoint, root, path)
        if symbol:
            symbols.append(symbol)
    return symbols


def _native_symbols(endpoint: str, root: Path) -> list[Symbol]:
    """读取端的原生索引；不认识的格式交给 HTML 扫描处理。"""

    parsers = {
        "server": parse_rust_search_index,
        "client": parse_javadoc_search_index,
        "agent": parse_typedoc_search_index,
    }
    parser = parsers[endpoint]
    patterns = {
        "server": ("search-index.js", "search.index/*.js"),
        "client": ("*-search-index.js", "type-search-index.js", "member-search-index.js"),
        "agent": ("assets/search.js", "assets/search-index.js", "**/assets/search.js", "**/assets/search-index.js"),
    }
    symbols: list[Symbol] = []
    for pattern in patterns[endpoint]:
        for path in sorted(root.glob(pattern)):
            for entry in parser(path.read_text(encoding="utf-8", errors="replace")):
                symbols.append(
                    Symbol(
                        endpoint,
                        entry["name"].split("::")[-1],
                        entry["qualified"],
                        entry["kind"],
                        f"{endpoint}/{entry['url'].lstrip('./')}",
                    )
                )
    return symbols


def _merge_symbols(native: list[Symbol], scanned: list[Symbol]) -> list[Symbol]:
    """按端点、名称和链接去重，优先保留原生索引的语义信息。"""

    merged: dict[tuple[str, str, str], Symbol] = {}
    for symbol in [*scanned, *native]:
        key = (symbol.endpoint, symbol.name, symbol.url)
        merged[key] = symbol
    return sorted(merged.values(), key=lambda item: (item.name.casefold(), item.qualified.casefold()))


def build_manifest(output: Path) -> dict[str, Any]:
    """构造入口页使用的端点、缺失状态和统一符号清单。"""

    endpoints: list[dict[str, Any]] = []
    for endpoint, label in (
        ("server", "Server · Rustdoc"),
        ("client", "Client · Javadoc"),
        ("agent", "Agent · TypeDoc"),
    ):
        root = output / endpoint
        scanned = _scan_html(endpoint, root)
        native = _native_symbols(endpoint, root) if root.is_dir() else []
        symbols = [asdict(symbol) for symbol in _merge_symbols(native, scanned)]
        endpoints.append(
            {
                "id": endpoint,
                "label": label,
                "available": root.is_dir() and bool(scanned),
                "path": f"{endpoint}/index.html",
                "symbolCount": len(symbols),
                "symbols": symbols,
            }
        )
    return {
        "version": 1,
        "endpoints": endpoints,
        "symbolCount": sum(endpoint["symbolCount"] for endpoint in endpoints),
    }


def render_index(manifest: dict[str, Any]) -> str:
    """渲染不依赖服务器、可由 ``file://`` 打开的统一搜索页。"""

    payload = html.escape(json.dumps(manifest, ensure_ascii=False), quote=False)
    return f"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Bong 代码 Wiki</title>
  <style>
    :root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
    body {{ max-width: 1100px; margin: 2rem auto; padding: 0 1rem; line-height: 1.5; }}
    input {{ width: 100%; box-sizing: border-box; padding: .7rem; font-size: 1.05rem; }}
    .endpoints {{ display: flex; gap: .6rem; flex-wrap: wrap; margin: 1rem 0; }}
    .endpoint {{ border: 1px solid #8885; border-radius: .5rem; padding: .5rem .8rem; }}
    .missing {{ color: #b44; }}
    ul {{ padding-left: 1.2rem; }}
    li {{ margin: .35rem 0; }}
    code {{ font-size: .9em; }}
  </style>
</head>
<body>
  <h1>Bong 代码 Wiki</h1>
  <p>本页由 <code>scripts/wiki/build.sh</code> 生成，可直接用浏览器打开。</p>
  <input id="search" type="search" placeholder="搜索函数、类型、类、接口或常量" autofocus>
  <div id="endpoints" class="endpoints"></div>
  <p id="summary"></p>
  <ul id="results"></ul>
  <script id="wiki-data" type="application/json">{payload}</script>
  <script>
    const data = JSON.parse(document.getElementById('wiki-data').textContent);
    const input = document.getElementById('search');
    const results = document.getElementById('results');
    const summary = document.getElementById('summary');
    const endpointBox = document.getElementById('endpoints');
    for (const endpoint of data.endpoints) {{
      const item = document.createElement('span');
      item.className = endpoint.available ? 'endpoint' : 'endpoint missing';
      item.textContent = endpoint.available
        ? `${{endpoint.label}} · ${{endpoint.symbolCount}} 个符号`
        : `${{endpoint.label}} · 生成物缺失`;
      endpointBox.appendChild(item);
    }}
    function showResults() {{
      const query = input.value.trim().toLocaleLowerCase();
      const all = data.endpoints.flatMap(endpoint => endpoint.symbols);
      const matches = query
        ? all.filter(symbol => `${{symbol.name}} ${{symbol.qualified}} ${{symbol.kind}}`.toLocaleLowerCase().includes(query))
        : [];
      results.replaceChildren();
      summary.textContent = query ? `找到 ${{matches.length}} 个结果` : '输入名称开始搜索';
      for (const symbol of matches.slice(0, 200)) {{
        const row = document.createElement('li');
        const link = document.createElement('a');
        link.href = symbol.url;
        link.textContent = symbol.qualified || symbol.name;
        row.append(link, document.createTextNode(` · ${{symbol.kind}} · ${{symbol.endpoint}}`));
        results.appendChild(row);
      }}
    }}
    input.addEventListener('input', showResults);
    showResults();
  </script>
</body>
</html>
"""


def write_site(output: Path) -> dict[str, Any]:
    """写入 JSON 清单与 HTML 入口，并返回统计信息。"""

    output.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(output)
    (output / "search-index.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (output / "index.html").write_text(render_index(manifest), encoding="utf-8")
    return manifest


def main() -> None:
    """命令行入口。"""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="三端文档的共同输出目录")
    args = parser.parse_args()
    manifest = write_site(args.output)
    for endpoint in manifest["endpoints"]:
        status = "ok" if endpoint["available"] else "missing"
        print(f"wiki endpoint={endpoint['id']} status={status} symbols={endpoint['symbolCount']}")
    print(f"wiki total symbols={manifest['symbolCount']}")


if __name__ == "__main__":
    main()
