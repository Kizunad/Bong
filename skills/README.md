# skills

本目录存放 `.pi/` `.claude/` 共享的 agent skill 定义。`.opencode/` 无 skills 概念，不需要映射。

## 结构

```
skills/                    ← 唯一 canonical（直接改这里）
├── audit-plans-progress/  ← 审计计划进度
├── gen-image/             ← 生成图片
├── library-lore/          ← 图书馆 lore 查阅
├── plans-status/          ← 计划状态总览
├── pr-watch/              ← PR 监控
├── review-book/           ← 书籍审查
├── write-book/            ← 书籍撰写
└── ...                    ← 后续新增 skill 放这里
```

## symlink 映射

```
.pi/skills/     → ../skills/
.claude/skills/ → ../skills/
```

## 约定

- 所有 skill 的 **唯一真实副本** 在 `skills/`
- `.pi/skills/` `.claude/skills/` 为目录级 symlink，新增/删除子目录自动跟随
- 新增 skill：直接在 `skills/` 下创建 `skill-name/SKILL.md`
- 删除 skill：直接 `rm -rf skills/<name>/`
- **禁止** 通过 symlink 路径编辑文件——统一在 `skills/` 下操作
