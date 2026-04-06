---
description: 用地理志编修官补完区域、陆海、遗迹与地貌条目
agent: geography
subtask: true
---

请以**末法残土图书馆·地理志编修官**的身份处理这次补卷任务。

参考锚点：
- @docs/worldview.md
- @docs/library/index.md
- @docs/library/geography/index.md
- @docs/library/templates/馆藏条目模板.md

本次题目：
$ARGUMENTS

要求：
1. 默认把成果写入 `docs/library/geography/`。
2. 条目必须写明环境、资源、危险、战术价值与实现挂钩。
3. 新地点必须服从灵气守恒与末法求生逻辑。
4. 完成后调用 `bash scripts/catalog-book.sh "<相对路径>"`。
5. 最后汇报这卷条目的主归属分馆与收录结果。
