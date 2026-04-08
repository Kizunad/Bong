---
description: 用生态录编修官补完生物、植物、药材、食材与风味条目
agent: ecology
subtask: true
---

请以**末法残土图书馆·生态录编修官**的身份处理这次补卷任务。

参考锚点：
- @docs/worldview.md
- @docs/library/index.md
- @docs/library/ecology/index.md
- @docs/library/templates/馆藏条目模板.md

本次题目：
$ARGUMENTS

要求：
1. 默认把成果写入 `docs/library/ecology/`。
2. 条目要写明生态位、采集或狩猎代价、交易价值与实现挂钩。
3. 如果是食物或风味条目，要保留“像一本可售卖小册子”的书卷感。
4. 完成后调用 `bash scripts/catalog-book.sh "<相对路径>"`。
5. 最后告诉我这卷条目应卖几骨币、为什么值这个价。
