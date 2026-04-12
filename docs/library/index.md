# 末法残土图书馆总目

> “骨币会贬，记载不朽。凡可立卷者，皆应先入馆藏，再论买卖与实现。”

---

## 使用方式

1. 以 `docs/library/templates/馆藏条目模板.md` 为底本，写入对应分馆。
2. 新条目写完后，运行：`bash scripts/catalog-book.sh "docs/library/<分馆>/<书名>.md"`。
3. 某个实现项落地后，运行：`bash scripts/mark-implemented.sh "docs/library/<分馆>/<书名>.md" "<实现项>"`。
4. 如果你批量整理过馆藏，或索引看起来不对，再运行：`bash scripts/rebuild-library-index.sh`。

---

## 分馆总览

> 本页由 `scripts/rebuild-library-index.sh` 自动生成。

| 分馆 | 收录册数 | 落地进度 | 索引 |
|---|---|---|---|
| 世界总志 | 1 | 0/4 | [进入](./world/index.md) |
| 地理志 | 1 | 0/4 | [进入](./geography/index.md) |
| 众生谱 | 2 | 1/15 | [进入](./peoples/index.md) |
| 生态录 | 1 | 0/12 | [进入](./ecology/index.md) |
| 修行藏 | 2 | 0/8 | [进入](./cultivation/index.md) |
