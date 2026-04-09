#!/usr/bin/env node
/**
 * 一次性脚本：将 docs/library/ 下的 .md 馆藏转为 .json 格式
 * 用法: node scripts/convert-library-to-json.mjs
 */

import fs from "node:fs";
import path from "node:path";

const LIBRARY = path.resolve("docs/library");

// ── 解析单本书 ──────────────────────────────────────

function convertBook(raw, relPath) {
  const lines = raw.split("\n");

  // 1. 标题
  const titleLine = lines.find((l) => /^#\s/.test(l)) || "";
  const title = titleLine.replace(/^#\s+/, "").replace(/[《》]/g, "").trim();

  // 2. 卷首引语
  let quote = "";
  for (const l of lines) {
    const m = l.match(/^>\s*"(.+)"$/);
    if (m) { quote = m[1]; break; }
  }

  // 3. 编目信息
  const catalog = {};
  const catalogMap = {
    "分馆": "hall", "书架": "shelf", "藏书编号": "id",
    "估值": "value", "稀有度": "rarity", "收录状态": "status",
    "锚点来源": "anchor", "收录时间": "date", "最后整理": "lastEdit",
  };
  let inCatalog = false;
  for (const l of lines) {
    if (/^##\s*编目信息/.test(l)) { inCatalog = true; continue; }
    if (inCatalog && /^##\s/.test(l)) break;
    if (inCatalog) {
      const kv = l.match(/^-\s*(.+?)：(.+)$/);
      if (kv) {
        const key = catalogMap[kv[1].trim()] || kv[1].trim();
        catalog[key] = kv[2].trim();
      }
    }
  }

  // 4. 找到各大段落的位置
  const h2Positions = [];
  for (let i = 0; i < lines.length; i++) {
    if (/^##\s/.test(lines[i])) {
      const heading = lines[i].replace(/^##\s+/, "").trim();
      h2Positions.push({ line: i, heading });
    }
  }

  // 5. 提取摘要
  let summary = "";
  const summaryIdx = h2Positions.findIndex((h) => h.heading === "摘要");
  if (summaryIdx >= 0) {
    const start = h2Positions[summaryIdx].line + 1;
    const end = h2Positions[summaryIdx + 1]?.line ?? lines.length;
    summary = extractBlock(lines, start, end);
  }

  // 6. 提取正文 sections（按 ### 分割）
  const sections = [];
  const bodyIdx = h2Positions.findIndex((h) => h.heading === "正文");
  if (bodyIdx >= 0) {
    const bodyStart = h2Positions[bodyIdx].line + 1;
    const bodyEnd = h2Positions[bodyIdx + 1]?.line ?? lines.length;
    const bodyLines = lines.slice(bodyStart, bodyEnd);

    let currentTitle = null;
    let currentBody = [];

    for (const l of bodyLines) {
      const h3 = l.match(/^###\s+(.+)/);
      if (h3) {
        if (currentTitle !== null) {
          sections.push(finishSection(currentTitle, currentBody));
        }
        currentTitle = h3[1].trim();
        currentBody = [];
      } else {
        currentBody.push(l);
      }
    }
    if (currentTitle !== null) {
      sections.push(finishSection(currentTitle, currentBody));
    }
  }

  // 7. 提取残卷引语（从 sections 中分离）
  const fragments = [];
  const fragmentSectionIdx = sections.findIndex(
    (s) => /残卷引语/.test(s.title) || /残卷/.test(s.title) && /引语/.test(s.title)
  );

  if (fragmentSectionIdx >= 0) {
    const fragSection = sections.splice(fragmentSectionIdx, 1)[0];
    // 按 h4 (####) 拆分
    const fragLines = fragSection.body.split("\n");
    let fTitle = null;
    let fBody = [];

    for (const l of fragLines) {
      const h4 = l.match(/^####\s+(.+)/);
      if (h4) {
        if (fTitle) fragments.push(makeFragment(fTitle, fBody));
        fTitle = h4[1].trim();
        fBody = [];
      } else {
        fBody.push(l);
      }
    }
    if (fTitle) fragments.push(makeFragment(fTitle, fBody));
  }

  // 8. 提取实现挂钩
  const implIdx = h2Positions.findIndex((h) => /实现挂钩/.test(h.heading));
  let implementation = null;
  if (implIdx >= 0) {
    const start = h2Positions[implIdx].line + 1;
    const implBlock = extractBlock(lines, start, lines.length);
    implementation = parseImplementation(implBlock);
  }

  // 9. 交叉引用（从实现挂钩中提取）
  const crossRefs = [];
  if (implementation?.notes) {
    const refMatches = implementation.notes.matchAll(/\[([《》\w·\s]+)\]\([^)]+\)/g);
    for (const m of refMatches) crossRefs.push(m[1]);
  }
  // 也从正文中提取
  const fullText = sections.map((s) => s.body).join("\n");
  const bodyRefs = fullText.matchAll(/\[([《》\w·\s]+)\]\([^)]+\)/g);
  for (const m of bodyRefs) {
    if (!crossRefs.includes(m[1])) crossRefs.push(m[1]);
  }

  // 10. 过滤掉纯技术的 sections
  const techKeywords = ["Bevy", "ECS", "组件结构", "系统函数", "Component", "对接"];
  const filteredSections = [];
  const techSections = [];

  for (const s of sections) {
    if (techKeywords.some((kw) => s.title.includes(kw))) {
      techSections.push(s);
    } else {
      filteredSections.push(s);
    }
  }

  // 技术 section 内容移入 implementation
  if (techSections.length > 0 && implementation) {
    const techContent = techSections
      .map((s) => `### ${s.title}\n\n${s.body}`)
      .join("\n\n---\n\n");
    implementation.notes = (implementation.notes || "") + "\n\n" + techContent;
  }

  return {
    title,
    quote,
    catalog,
    summary,
    sections: filteredSections,
    fragments: fragments.length > 0 ? fragments : undefined,
    crossRefs: crossRefs.length > 0 ? crossRefs : undefined,
    implementation: implementation || undefined,
  };
}

function extractBlock(lines, start, end) {
  const block = lines.slice(start, end);
  // trim leading/trailing blank lines and ---
  while (block.length && /^(\s*|---)$/.test(block[0])) block.shift();
  while (block.length && /^(\s*|---)$/.test(block[block.length - 1])) block.pop();
  return block.join("\n");
}

function finishSection(title, bodyLines) {
  while (bodyLines.length && /^(\s*|---)$/.test(bodyLines[0])) bodyLines.shift();
  while (bodyLines.length && /^(\s*|---)$/.test(bodyLines[bodyLines.length - 1])) bodyLines.pop();
  return { title, body: bodyLines.join("\n") };
}

function makeFragment(title, bodyLines) {
  while (bodyLines.length && /^\s*$/.test(bodyLines[0])) bodyLines.shift();
  while (bodyLines.length && /^\s*$/.test(bodyLines[bodyLines.length - 1])) bodyLines.pop();

  // 分离引语和备注
  const quoteLines = [];
  const noteLines = [];
  let inQuote = true;
  for (const l of bodyLines) {
    if (inQuote && /^>/.test(l)) {
      quoteLines.push(l.replace(/^>\s?/, ""));
    } else if (inQuote && /^\s*$/.test(l)) {
      quoteLines.push("");
    } else {
      inQuote = false;
      noteLines.push(l);
    }
  }

  const text = quoteLines
    .join("\n")
    .replace(/^"/, "").replace(/"$/, "")
    .trim();
  const note = noteLines.join("\n").replace(/^\*/, "").replace(/\*$/, "").trim();

  const frag = { title, text };
  if (note) frag.note = note;
  return frag;
}

function parseImplementation(block) {
  const modules = [];
  const files = [];
  const todos = [];
  const lines = block.split("\n");
  const noteLines = [];
  let collectNotes = false;

  for (const l of lines) {
    // 关联模块
    const modMatch = l.match(/关联模块[：:]\s*(.+)/);
    if (modMatch) {
      modules.push(...modMatch[1].split(/[/／、,]/).map((s) => s.trim()).filter(Boolean));
      continue;
    }
    // 关联文件
    const fileMatch = l.match(/^\s+-\s+`([^`]+)`/);
    if (fileMatch) { files.push(fileMatch[1]); continue; }
    // 交叉引用（文件形式）
    const refMatch = l.match(/^\s+-\s+交叉引用/);
    if (refMatch) continue;
    // Todo items
    const todoMatch = l.match(/^\s*-\s*\[([x ])\]\s+(.+)/);
    if (todoMatch) {
      todos.push({ done: todoMatch[1] === "x", text: todoMatch[2] });
      continue;
    }
    // 设计提案等
    if (/^###\s/.test(l)) collectNotes = true;
    if (collectNotes) noteLines.push(l);
  }

  return {
    modules: modules.length ? modules : undefined,
    files: files.length ? files : undefined,
    todos: todos.length ? todos : undefined,
    notes: noteLines.length ? noteLines.join("\n").trim() : undefined,
  };
}

// ── 主流程 ────────────────────────────────────────

const skipFiles = ["index.md", "交叉引用指南.md"];
const skipDirs = ["templates"];

function walk(dir) {
  const results = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (skipDirs.includes(entry.name)) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...walk(full));
    } else if (entry.name.endsWith(".md") && !skipFiles.includes(entry.name)) {
      results.push(full);
    }
  }
  return results;
}

const mdFiles = walk(LIBRARY);
console.log(`找到 ${mdFiles.length} 本馆藏 MD`);

for (const mdPath of mdFiles) {
  const raw = fs.readFileSync(mdPath, "utf-8");
  const rel = path.relative(LIBRARY, mdPath);
  const book = convertBook(raw, rel);

  const jsonPath = mdPath.replace(/\.md$/, ".json");
  fs.writeFileSync(jsonPath, JSON.stringify(book, null, 2), "utf-8");
  console.log(`  [ok] ${rel} → ${path.basename(jsonPath)}`);
}

console.log("\n转换完成。旧 .md 文件未删除，确认无误后手动删除。");
