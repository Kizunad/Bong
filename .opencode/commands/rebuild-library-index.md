---
description: 用图书馆总编目官重建总目与分馆索引
agent: library-curator
subtask: true
---

请以**末法残土图书馆总编目官**的身份重建图书馆索引。

参数：
$ARGUMENTS

要求：
1. 如果我传了参数，就把它当作分馆 slug（如 `world` / `geography` / `peoples` / `ecology` / `cultivation`）。
2. 如果我没传参数，就重建全馆总目与全部分馆索引。
3. 调用：`bash scripts/rebuild-library-index.sh $ARGUMENTS`
4. 最后汇报重建了哪些索引文件，以及当前收录册数概况。
