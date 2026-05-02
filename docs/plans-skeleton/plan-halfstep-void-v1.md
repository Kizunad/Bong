# Bong · plan-halfstep-void-v1 · 骨架

**主题**：半步化虚 buff 实装 + 重渡机制

`DuXuOutcomeV1::HalfStep` 已在渡虚劫结算流程中正确返回（`server/src/cultivation/tribulation.rs:1128`），但两件事仍为占位：① buff（+10% 真元上限 / +200 年寿元）**未实装**——HalfStep 路径只返回 enum 值，没有修改 `qi_max` 或 `LifespanComponent`；② 名额空出时的**重渡路径**（通知 + 玩家确认 + 起劫）未设计。本 plan 补全这两块。

**世界观锚点**：`worldview.md §三 line 72`（化虚 500 真元上限，服务器仅容 1-2 人；超额者扛过天劫也"半步"，天道不赐满化虚——但也不永远堵死）· `§八 天道行为准则`（天道冷漠，名额空出时只广播事实，不主动引导）

**交叉引用**：`plan-tribulation-v1`（渡虚劫主链，HalfStep 结算入口）· `plan-void-quota-v1`（化虚名额动态计算，未来可替换当前 player_count/50 公式）· `plan-lifespan-v1`（寿元精细化，bonus 加成接口）· `plan-death-lifecycle-v1`（重渡失败走普通退境路径）

---

## 接入面 Checklist

- **进料**：`TribulationSettled { outcome: HalfStep }` → `AscensionQuotaOpened { occupied_slots }` 触发重渡通知
- **出料**：`HalfStepBuff` component → `cultivation_buffs` DB 持久化 → client HUD 半步标记 → 重渡流程结束后升化虚 / 再次半步（不叠加）
- **共享类型**：`DuXuOutcomeV1::HalfStep`（已有）· `AscensionQuotaOpened`（已有）· `HalfStepQuotaNotifyV1`（新增，仅发给在线半步玩家）
- **跨仓库契约**：server buff 实装 + DB 持久化 → client HUD 显示半步标记 + 重渡确认 UI → agent narration（重渡成功/失败专属语调）
- **worldview 锚点**：§三 + §八

---

## §0 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | HalfStep buff 实装：结算时 `qi_max *= 1.10` + `lifespan.bonus_yr += 200`，持久化进 `cultivation_buffs` 表 | 单测覆盖：buff 数值正确；重连后 buff 保留；化虚者不触发 |
| **P1** ⬜ | 重渡通知：`AscensionQuotaOpened` 触发时扫描在线半步玩家 → 定向发 `HalfStepQuotaNotifyV1` | 在线半步玩家收到专属包；非半步玩家只收通用 quota 广播；单测验证包路由 |
| **P2** ⬜ | 重渡流程：client 确认 UI → server 校验（半步状态 + quota 空位）→ 启动新渡虚劫（波数 −1，心魔减一档） → 成功移除半步 buff 升化虚 → 失败退通灵初期 | 端到端：重渡成功化虚；重渡失败退境；quota 再次满时重渡仍→半步（buff 不叠加，reset） |
| **P3** ⬜ | Config 化 buff 强度：`qi_max_multiplier` / `lifespan_bonus_yr` 从 `game_config` 读取，运维可调不重编译 | config 字段定义；默认值与 P0 一致；单测覆盖 override 路径 |

---

## §1 数据契约

- [ ] `server/src/cultivation/tribulation_buff.rs` — `HalfStepBuff { qi_max_multiplier: f32, lifespan_bonus_yr: f32 }` component + apply/remove 函数
- [ ] `server/src/persistence/schema.sql` — `cultivation_buffs` 表（`char_id TEXT, buff_kind TEXT, f32_value REAL, created_at INT`）或现有 buff 表扩展
- [ ] `server/src/schema/server_data.rs` — `HalfStepQuotaNotifyV1`（`quota_now: u32, quota_max: u32`）payload，仅在 half-step 玩家在线时定向发
- [ ] `server/src/network/ascension_quota_emit.rs` — 在 `AscensionQuotaOpened` 处理路径中增加 half-step 玩家扫描 + 定向发
- [ ] `agent/packages/tiandao/` — 重渡成功（"半步化虚者，再叩天门"）/ 失败（"又一次门缝闭合"）专属 narration
- [ ] `client/.../hud/` — 通灵境玩家 HUD 增加"半步"角标（半步 buff 存在时显示）

---

## §2 关键约束

- **buff 默认值**（P3 前硬编码，P3 后 config）：`qi_max_multiplier = 1.10`（通灵满 300 → 330）· `lifespan_bonus_yr = 200.0`
- **重渡惩罚**：与普通渡虚劫相同（失败退通灵初期），不因曾半步而有额外惩罚或优待（波数 −1 是对"已历劫者"的小幅适配，不是免死金牌）
- **quota 再满时**：重渡成功仍→半步，buff 不叠加（reset 到基准值而非累加）
- **半步降境**：buff 保留（天道无法收回已给出的半步之力，但降境后真元上限按新境界重算，回通灵时 buff 倍率重新计入通灵圆满上限）

---

## §3 开放问题

- [ ] 重渡波数 −1 是否合理？若原渡劫 3 波，重渡为 2 波，是否过于简单？（倾向：保持 min=2，3 波渡劫降为 2 波，5 波降为 4 波）
- [ ] `AscensionQuotaOpened` 广播到离线半步玩家的处理：下次上线时补发通知？还是只实时推？（倾向：仅实时，上线时 client inspect 可查当前名额）
- [ ] `lifespan.bonus_yr` 的接口：`plan-lifespan-v1` 尚在骨架阶段，当前 `LifespanComponent` 是否已有 bonus 字段？若无，P0 需临时加字段
- [ ] config 存储位置：`game_config.toml` 静态文件还是 DB `server_config` 表（支持热更）？

---

## §4 进度日志

- 2026-05-02：骨架创建。源自 `plans-skeleton/reminder.md` plan-tribulation-v1 段（buff 占位 + 重渡机制）。代码核查：`DuXuOutcomeV1::HalfStep` 已有（`tribulation.rs:1128`），buff 完全未实装；`AscensionQuotaOpened` 广播全体客户端但无定向半步通知逻辑。
