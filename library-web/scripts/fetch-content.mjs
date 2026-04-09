/**
 * 从 GitHub 私有仓库（或本地）拉取 docs/library/ 下的 .json 馆藏，
 * 转换为 Astro content collection 的 .md 文件（YAML frontmatter + markdown body）。
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.resolve(__dirname, "../src/content/books");

const GITHUB_TOKEN = process.env.GITHUB_TOKEN;
const GITHUB_OWNER = process.env.GITHUB_OWNER || "Kizunad";
const GITHUB_REPO = process.env.GITHUB_REPO || "Bong";
const GITHUB_BRANCH = process.env.GITHUB_BRANCH || "main";
const LOCAL_PATH = process.env.LOCAL_LIBRARY_PATH;

const HALL_NAMES = {
  world: "世界总志", geography: "地理志", peoples: "众生谱",
  ecology: "生态录", cultivation: "修行藏",
};
const HALL_DESCS = {
  world: "天地初开至末法降临，万年兴衰总录",
  geography: "残土山川、秘境裂隙、灵脉分布",
  peoples: "宗门遗脉、散修百态、异兽图鉴",
  ecology: "灵草药材、毒物异种、生态链",
  cultivation: "功法残篇、境界注解、修行笔记",
};

// ── GitHub API ────────────────────────────────────

async function ghFetch(urlPath) {
  const res = await fetch(`https://api.github.com${urlPath}`, {
    headers: {
      Authorization: `Bearer ${GITHUB_TOKEN}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status}: ${urlPath}`);
  return res.json();
}

async function fetchTreePaths() {
  const tree = await ghFetch(
    `/repos/${GITHUB_OWNER}/${GITHUB_REPO}/git/trees/${GITHUB_BRANCH}?recursive=1`
  );
  return tree.tree
    .filter((n) =>
      n.type === "blob" &&
      n.path.startsWith("docs/library/") &&
      n.path.endsWith(".json") &&
      !n.path.includes("templates/")
    )
    .map((n) => n.path);
}

async function fetchFileContent(filePath) {
  const data = await ghFetch(
    `/repos/${GITHUB_OWNER}/${GITHUB_REPO}/contents/${encodeURIComponent(filePath)}?ref=${GITHUB_BRANCH}`
  );
  return Buffer.from(data.content, "base64").toString("utf-8");
}

// ── 本地读取 ──────────────────────────────────────

function localTreePaths(baseDir) {
  const results = [];
  function walk(dir, rel) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === "templates") continue;
      const full = path.join(dir, entry.name);
      const relPath = path.join(rel, entry.name);
      if (entry.isDirectory()) walk(full, relPath);
      else if (entry.name.endsWith(".json")) results.push(relPath);
    }
  }
  walk(baseDir, "");
  return results;
}

// ── JSON → Astro Markdown ─────────────────────────

function bookToMarkdown(book, hallSlug) {
  // frontmatter
  const fm = {
    title: book.title || "无题",
    hall: book.catalog?.hall || HALL_NAMES[hallSlug] || hallSlug,
    hallSlug,
    shelf: book.catalog?.shelf || "",
    catalogId: book.catalog?.id || "",
    value: book.catalog?.value || "",
    rarity: book.catalog?.rarity || "常见",
    status: book.catalog?.status || "待收录",
    quote: book.quote || "",
    anchor: book.catalog?.anchor || "",
    catalogDate: book.catalog?.date || "",
    lastEdit: book.catalog?.lastEdit || "",
  };

  const yamlLines = ["---"];
  for (const [k, v] of Object.entries(fm)) {
    yamlLines.push(`${k}: ${JSON.stringify(v)}`);
  }
  yamlLines.push("---", "");

  // body
  const body = [];

  // 摘要
  if (book.summary) {
    body.push("## 摘要", "", book.summary, "", "---", "");
  }

  // 正文 sections
  for (const sec of book.sections || []) {
    body.push(`## ${sec.title}`, "", sec.body, "", "---", "");
  }

  // 残卷引语
  if (book.fragments?.length) {
    body.push("## 残卷引语", "");
    for (const frag of book.fragments) {
      body.push(`### ${frag.title}`, "");
      body.push(`> ${frag.text.split("\n").join("\n> ")}`, "");
      if (frag.note) {
        body.push(`*${frag.note}*`, "");
      }
      body.push("");
    }
  }

  return yamlLines.join("\n") + "\n" + body.join("\n");
}

// ── 主流程 ────────────────────────────────────────

async function main() {
  if (fs.existsSync(OUT_DIR)) fs.rmSync(OUT_DIR, { recursive: true });

  let files; // { path, content }[]

  if (LOCAL_PATH) {
    const base = path.resolve(LOCAL_PATH);
    console.log(`[fetch] 从本地读取: ${base}`);
    const paths = localTreePaths(base);
    files = paths.map((rel) => ({
      path: rel,
      content: fs.readFileSync(path.join(base, rel), "utf-8"),
    }));
  } else {
    if (!GITHUB_TOKEN) {
      console.error("[fetch] 需要 GITHUB_TOKEN 或 LOCAL_LIBRARY_PATH");
      process.exit(1);
    }
    console.log(`[fetch] 从 GitHub 拉取: ${GITHUB_OWNER}/${GITHUB_REPO}@${GITHUB_BRANCH}`);
    const treePaths = await fetchTreePaths();
    files = [];
    for (const p of treePaths) {
      const rel = p.replace(/^docs\/library\//, "");
      const content = await fetchFileContent(p);
      files.push({ path: rel, content });
    }
  }

  console.log(`[fetch] 共 ${files.length} 本馆藏`);

  // hall 元数据
  const hallMeta = {};
  for (const [slug, name] of Object.entries(HALL_NAMES)) {
    hallMeta[slug] = {
      name,
      desc: HALL_DESCS[slug] || "",
      bookCount: files.filter((f) => f.path.startsWith(slug + "/") || f.path.startsWith(slug + "\\")).length,
    };
  }
  const libDir = path.resolve(__dirname, "../src/lib");
  fs.mkdirSync(libDir, { recursive: true });
  fs.writeFileSync(path.join(libDir, "halls.json"), JSON.stringify(hallMeta, null, 2));

  // 处理每本书
  for (const file of files) {
    let book;
    try {
      book = JSON.parse(file.content);
    } catch (e) {
      console.error(`  [跳过] ${file.path}: JSON 解析失败`);
      continue;
    }

    const parts = file.path.split(/[/\\]/);
    const hallSlug = parts.length >= 2 ? parts[0] : "world";
    const mdName = parts[parts.length - 1].replace(/\.json$/, ".md");
    const md = bookToMarkdown(book, hallSlug);

    const outPath = path.join(OUT_DIR, hallSlug, mdName);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, md, "utf-8");
    console.log(`  [ok] ${file.path}`);
  }

  console.log("[fetch] 完成");
}

main().catch((err) => {
  console.error("[fetch] 失败:", err);
  process.exit(1);
});
