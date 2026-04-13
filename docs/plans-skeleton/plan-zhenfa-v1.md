# Bong · plan-zhenfa-v1 · 模板

**阵法专项**。HUD §8.2 提到 "Dynamic XML Screen 阵法布置"，combat 引用 Zhenfa 技能，本 plan 定义：布阵 / 触发 / 破阵 / 阵法种类。

**交叉引用**：`plan-combat-v1.md` · `plan-client.md`（DynamicXmlScreen）· `plan-worldgen-v3.1.md`。

---

## §0 设计轴心

- [ ] 阵法 = 空间持久增益/陷阱
- [ ] 布阵需材料 + 时间 + 真元
- [ ] 可见 / 隐蔽
- [ ] 可被破解（阵眼）

## §1 阵法种类

| 类型 | 用途 | 范围 | 代表 |
|---|---|---|---|
| 聚灵阵 | 加速修炼 | 小 | |
| 防御阵 | 护山 / 护身 | 中/大 | |
| 幻阵 | 迷惑入侵者 | 中 | |
| 杀阵 | 陷阱 | 小/中 | |
| 传送阵 | 跨地图 | 点 | |

## §2 布阵流程

```
选阵图 → 选点 → 消耗材料 → 布阵动画 → 阵成
```

- [ ] DynamicXmlScreen 布阵 UI
- [ ] 阵旗 / 阵眼实体（server ECS）
- [ ] 布阵者权限

## §3 触发与效果

- [ ] 进入触发 / 主动触发 / 条件触发
- [ ] 效果叠加规则
- [ ] 与玩家 StatusEffect 的交互

## §4 破阵

- [ ] 寻找阵眼
- [ ] 破阵技能 / 阵图识别
- [ ] 失败代价

## §5 数据契约

- [ ] ZhenfaInstance / ZhenfaRegistry
- [ ] server 持久化（存档一部分）
- [ ] Channel

## §6 实施节点

## §7 开放问题
