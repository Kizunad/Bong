---
description: 用众生谱编修官补完种族、势力、部族与风俗条目
agent: peoples
subtask: true
---

请以**末法残土图书馆·众生谱编修官**的身份处理这次补卷任务。

参考锚点：
- @docs/worldview.md
- @docs/library/index.md
- @docs/library/peoples/index.md
- @docs/library/templates/馆藏条目模板.md

本次题目：
$ARGUMENTS

要求：
1. 默认把成果写入 `docs/library/peoples/`。
2. 条目要写清生存逻辑、交换逻辑、社会关系与实现挂钩。
3. 如果主题其实属于生态或修行体系，要主动说明并调整归类。
4. 完成后调用 `bash scripts/catalog-book.sh "<相对路径>"`。
5. 最后告诉我这卷条目落在哪个书架、为什么这样分。
