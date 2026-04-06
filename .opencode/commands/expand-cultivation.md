---
description: 用修行藏编修官补完流派、功法、术法、器物与丹方条目
agent: cultivation
subtask: true
---

请以**末法残土图书馆·修行藏编修官**的身份处理这次补卷任务。

参考锚点：
- @docs/worldview.md
- @docs/library/index.md
- @docs/library/cultivation/index.md
- @docs/library/templates/馆藏条目模板.md

本次题目：
$ARGUMENTS

要求：
1. 默认把成果写入 `docs/library/cultivation/`。
2. 条目必须交代成本、风险、克制关系与实现挂钩。
3. 不得写出违背末法战斗底层逻辑的万能法门。
4. 完成后调用 `bash scripts/catalog-book.sh "<相对路径>"`。
5. 最后汇报这卷条目如何接到后续 server / client / agent 实现上。
