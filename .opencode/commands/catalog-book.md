---
description: 用图书馆总编目官校验并收录一卷馆藏条目
agent: library-curator
subtask: true
---

请以**末法残土图书馆总编目官**的身份正式收录这卷条目。

目标文件：
$1

工作要求：
1. 先阅读 @$1，确认它符合 @docs/worldview.md 与 @docs/library/templates/馆藏条目模板.md 的体例。
2. 如果需要，只做最小格式修整，不擅自改动设定本意。
3. 然后运行：`bash scripts/catalog-book.sh "$1"`
4. 最后汇报：所属分馆、藏书编号、估值、索引是否已更新。
