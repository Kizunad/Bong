# 狮、鹫、马配置与验收

`wildlife.toml` 配置质量、移动倍率、撤退阈值、战术技能槽，以及两个出生境界的基础属性。境界沿用 `Realm`：Awaken=0 醒灵、Induce=1 引气、Condense=2 凝脉、Solidify=3 固元。

| 物种 | 境界 | 生命 | 基础攻击 | 承伤倍率 | 体力 |
|---|---|---|---|---|---|
| 呆怒狮 | 凝脉 / 固元 | 180 / 420 | 14 / 25 | 0.85 / 0.70 | 100 / 130 |
| 腐羽鹫 | 醒灵 / 引气 | 18 / 35 | 4 / 7 | 1.00 / 0.95 | 65 / 85 |
| 马 | 醒灵 / 引气 | 45 / 85 | 7 / 11 | 0.95 / 0.85 | 110 / 140 |

攻击值进入 `DerivedAttrs`，实际伤害还经过技能倍率、命中部位、护甲、防御和统一战斗结算。高境个体约占五分之一，固元狮仅在危险度至少 4 的区域出现。初始真元为零，随后通过共用吐纳系统从所在区域吸收。

## 经脉与技能

- `../body_plans/plans/{dainu_lion,fuyu_vulture,horse}.json`：非人形部位盒、经脉、连边、逐境配额与污染注入映射。
- `../body_plans/races.json`：种族与构型绑定。经脉 ID 使用各物种自己的 snake_case 名字。
- `../cultivation/techniques.toml`：技能的种族门、境界门、依赖经脉与最低完整度、真元/体力成本、前摇、冷却和距离。
- `../../src/fauna/wildlife/skills.rs`：通过既有 `SkillRegistry` 注册执行器。全新技能行为需要注册执行器；修改经脉依赖时同步 `declare_dependencies`，启动校验检查它与 TOML 的一致性。
- `wildlife.toml` 的 `engage_skill` / `followup_skill`：选择起手与接续技能，出生装入 `KnownTechniques`，仍受种族、境界和经脉门限制。

启动时会核对技能槽的元数据、执行器、种族门、可达境界和物种构型经脉。拼错技能 ID 或填入无法使用的异种技能会明确报错。

五招为 `lion.pounce`（扑击，1.5 倍）、`lion.rend`（撕咬，1 倍）、`vulture.dive`（俯冲，1.3 倍）、`horse.trample`（践踏，1.6 倍）、`horse.kick`（后踢，1.2 倍）。默认真元成本为 0；配置非零成本后，使用已有真元释放事务，失败不会扣体力或进入冷却。

## BigBrain 与生成

行为优先级为脱险、战斗、归巢、休息、闲逛。低生命或低体力会脱离；目标死亡、跨层、离巢过远或仇恨过期后放弃追击。

- 狮子警戒近身目标，饥饿时扩大捕猎距离。扑击前摇锁方向，伤害回执确认命中才接撕咬，扑空重新调整。
- 鹫偏好受伤的低境目标，展翼升空后绕行、俯冲、脱离，恢复期间不会连续贴身攻击。
- 马平时中立，受击后同层、同群且附近的马共享攻击者，集结后从不同侧位冲锋。落单会后踢或逃离。每只马的一次冲锋最多向锁定目标提交一次攻击。
- 地面移动复用 Navigator，冲锋复用地面扫掠。飞行逐段检查体积占用，未知区块视为阻挡，击退与控制效果优先。

环境刷新仅在主世界正灵气非出生区进行。狮单只，鹫和马每批最多三只；每个成员独立检查地表、玩家距离和预算，整批使用共同原点决定物种与群归属。超距回收先归还真元，再移除实体。

## 本地验证

启用既有 dev 模式后使用 `/ambient_spawn once dainu_lion <x> <z>`，或将种类换成 `fuyu_vulture`、`horse`。命令绕过自然刷新频率与选种，仍使用真实地面落点、构型、属性、技能和行为。附近连续生成的马可验证群体响应。

客户端 `/fauna-preview spawn <物种>` 与 `/fauna-preview play <动画>` 只预览模型动作。离线复核材料位于 `modelScript/out/client-creatures/review/`，外观比例和动作节奏还需要游戏内人工体验。

技能图标由 `scripts/images/gen_wildlife_skill_icons.py` 固定几何生成。音效配方在 server `assets/audio/recipes/` 与 client `assets/bong/audio_recipes/` 同步保存。旧生平卷的闭脉条目仍限人形枚举；非人形闭脉和永久断脉的权威状态已经生效，尚不产生该旧格式的文字条目。

测试维护：音效加载测试移除了固定目录总数断言，保留具体配方可加载契约并覆盖新增兽技和死亡音效，避免新增资源引发无关计数失败。命令树快照增加三种生物的真实生成命令。

### 2026-09-20 验证记录

合入主线 `0b6aff68b` 后完成以下复验；modelScript 全量结果来自合并前，本次主线未改动该目录。

- 根目录运行 `bash scripts/build-token.sh cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 与 `cargo test`（后两项同样使用 build-token）：全部通过。完整测试累计 12,579 项通过、6 项忽略，包含应用启动及外部集成目标。
- 物种契约覆盖真实生成、属性聚合、经脉损伤与断脉门控、真元事务回滚、统一伤害回执、同群仇恨、地面与飞行碰撞，以及 BigBrain 自主施法。马群测试同时验证中立和受击后集结；鹫必须实际爬升后才能俯冲。
- Java 17 下 `bash scripts/build-token.sh gradle test build` 通过；JUnit 5,078 项通过，GameTest 3 项通过。
- Agent workspace 的 `npm run build` 与 schema、tiandao 两包的 `npm test` 通过，分别为 913 项与 872 项测试。
- `python3 -m unittest discover -s modelScript/tests -p "test_*.py"`：861 项通过。生物导出与资源打包定向测试：8 项通过。
- 包内 15 套形态、153 条动画及相关音效和图标共 58 个文件与 client 源资源逐字节一致；ZIP、sidecar、manifest 和服务端的 SHA-1/大小一致。合并后资源包 SHA-1 为 `fb367e756839b36a9f492039072b6e7e04ee34bd`，大小为 74,243,905 字节，资源打包 5 项测试复验通过。

以上为代码与资源契约验证，未代替 Minecraft 内对模型比例、动作衔接和战斗手感的人工验收。鹅仍只有资源与预览；缝合兽核心/碎体和马负载动作已可播放，尚无对应的服务端状态事件。
