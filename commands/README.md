# commands

本目录存放 `.pi/` `.opencode/` `.claude/` 三方共享的斜杠命令（slash command）定义。

## 结构

```
commands/                 ← 唯一 canonical（直接改这里）
├── consume-plan.md       ← plan 消费流水线命令
├── ...                   ← 后续新增命令放这里
```

## symlink 映射

```
.pi/prompts/        → ../../commands/
.opencode/commands/ → ../../commands/
.claude/commands/   → ../../commands/
```

## 约定

- 所有命令的 **唯一真实副本** 在 `commands/`
- `.pi/prompts/` `.opencode/commands/` `.claude/commands/` 中的同名文件均为 symlink
- 新增命令：直接在 `commands/` 创建，然后在三个目标目录建 symlink
- 删除命令：删除 `commands/` 中文件 + 三个目标目录的 symlink
- **禁止** 在 `.pi/` `.opencode/` `.claude/` 下直接编辑 symlink 指向的文件（会被 git 跟踪为真实文件变更，但 symlink 本身不保护）——统一在 `commands/` 编辑
