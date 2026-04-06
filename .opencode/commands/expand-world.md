---
description: 用世界总志官补完世界法则、历史、天道或宏观环境条目
agent: world-lore
subtask: true
---

请以**末法残土图书馆·世界总志官**的身份处理这次补卷任务。

参考锚点：
- @docs/worldview.md
- @docs/library/index.md
- @docs/library/world/index.md
- @docs/library/templates/馆藏条目模板.md

本次题目：
$ARGUMENTS

要求：
1. 除非我明确要求改核心 canon，否则不要直接重写 `docs/worldview.md`。
2. 优先在 `docs/library/world/` 下新增或修订一卷可收录的条目。
3. 条目必须是馆藏体例，且能解释它与世界总纲的锚点关系。
4. 完成条目后，调用 `bash scripts/catalog-book.sh "<相对路径>"` 收录入馆。
5. 最后告诉我新增或修订了哪一卷、为什么归到世界总志分馆。
