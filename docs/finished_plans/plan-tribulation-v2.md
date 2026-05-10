# Bong · plan-tribulation-v2

**绝壁劫**——化虚级极端操作触发的超强天劫。承接 `plan-tribulation-v1` ✅ finished（渡虚劫/域崩/定向天罚全链路已落）+ `plan-void-quota-v1` ✅ finished（世界灵气预算名额，当前超额 = 瞬死）。v2 核心：**把"绝壁劫"从 100% 必死判决改为真正可渡但极难的超强天劫序列**，且 zone 级 AOE 造成"一损俱损"——威慑所有人不要逼化虚出绝招。

**设计哲学**：绝壁劫不是惩罚，是物理后果。化虚级极端操作（全 zone 引爆/全池散功/紊流死区）= 灵气大规模异变 → 天道自然响应 = 降超强天劫。跟渡虚劫的"考试"不同，绝壁劫是"你搅了天地，天地反震"。结果：**别逼化虚**。化虚者出绝招 = 自己大概率死 + 周围所有人一起遭殃。

**世界观锚点**：`worldview.md §三:78`（天道主动针对化虚修士）· `§三:187 ×5 质变`（化虚级不是又一级而是凡躯重铸）· `§三:386-389`（全力一击 = 极端孤注 + 化虚老怪全力出手也得调息半个时辰）· `§八`（天道语调冷漠嘲讽）

**library 锚点**：`cultivation-0002 烬灰子内观笔记`（灵气大规模异变 → 天象反应的物理依据）

---

## 接入面 Checklist

- **进料**：
  - `cultivation::tribulation::TribulationState`（v1 已建）→ 扩展新 Kind
  - `cultivation::void::actions`（VoidActionBacklash 当前瞬死）→ 改为触发绝壁劫
  - `cultivation::tribulation::check_void_quota`（超额瞬死）→ 改为触发绝壁劫
  - `combat::dugu_v2::cast_reverse`（倒蚀）→ emit 绝壁劫触发事件
  - `combat::baomai_v3::cast_disperse`（散功）→ emit 绝壁劫触发事件
  - `combat::woliu_v2::vortex_heart`（涡心）→ emit 绝壁劫触发事件
  - `world::karma::KarmaWeightStore`（天道注视）→ 绝壁劫强度修正因子
- **出料**：
  - `TribulationKind::JueBi` 新变体 → 复用 tribulation 整套阶段流程
  - `JueBiTriggeredEvent` 🆕 → agent narration + client HUD
  - zone 级 AOE 伤害（绝壁劫范围内所有实体）
  - `BiographyEntry::JueBiSurvived` / `BiographyEntry::JueBiKilled`（生平卷）
- **共享类型**：复用 `TribulationState` / `TribulationPhase`，不新建
- **跨仓库契约**：
  - server: `cultivation::tribulation` 扩 JueBi 变体 + zone AOE 系统
  - agent: `tiandao::juebi_runtime`（绝壁劫专属 narration，冷漠 + 嘲讽 + "天地反震"）
  - client: 绝壁劫天象 VFX（比渡虚劫更剧烈）+ HUD 预警 + zone 范围视觉
- **worldview 锚点**：§三:78 + §三:187 + §三:386 + §八
- **qi_physics 锚点**：绝壁劫 AOE 走 `qi_physics::collision` 同源伤害公式（强度 ×1.5 作为 intensity 参数），不自建公式

---

## §0 设计轴心

**绝壁劫 ≠ 必死判决，是超强天劫 + 一损俱损**

| 维度 | 渡虚劫（v1） | 绝壁劫（v2） |
|------|-------------|-------------|
| 性质 | 考试（主动起劫） | 反震（被动触发） |
| 强度 | 基线 | ×1.5 |
| AOE 范围 | 100 格 | zone 级（≥300 格） |
| 可规避 | 不起劫即不降 | 不做极端操作即不降 |
| 波次 | 3-5 波 | 2-3 波（急促剧烈） |
| 心魔 | 有（选项式） | 无（非考试，纯物理反震） |
| 存活率 | ~50%（看准备） | <10%（但理论可活） |
| 核心威慑 | "你准备好了吗" | "别逼化虚 / 一损俱损" |
| 观战伤害 | 100 格内同等强度 | zone 级衰减式（近处致命 / 远处重伤） |

---

## §1 触发条件

绝壁劫由 **天道注视急剧飙升** 触发——化虚级极端操作瞬间产生的灵气异变超过天道容忍阈值。

| 触发源 | 操作 | 延迟 | 备注 |
|--------|------|------|------|
| dugu-v2 倒蚀 | zone 内所有永久标记同时引爆 | 30s | 化虚毒蛊师最孤注的一击 |
| baomai-v3 散功 | 30 天内连续 ≥3 次散功 | 即时 | 每次 -50% qi_max，连续 = 反复逆天 |
| woliu-v2 涡心 | 化虚紊流死区 zone 级展开 | 30s | 整个山谷瞬间进入紊流死区 |
| void-quota 超额 | 名额满仍强行起劫 | 0s（渡虚劫结算时转） | 替代当前瞬死，改为绝壁劫序列 |
| zhenfa-v2 欺天阵被识破 | 天道发现被骗 | 10s | 反噬 ×3 + 绝壁劫 |
| 未来扩展 | 任何"天道注视 > 阈值"的事件 | 配置 | 通用接口 |

**不触发绝壁劫的情况**：
- 正常渡虚劫（那是渡虚劫不是绝壁劫）
- 域崩（那是区域事件不是针对个人）
- 化虚者日常存在（定向天罚是隐性概率，不是绝壁劫）
- 非化虚境的任何操作（低境界搅不动天地）

---

## §2 绝壁劫阶段流程（天地排异反应）

绝壁劫不是"天道降雷惩罚"——是化虚极端操作撕裂了局部灵气场，天地物理规则为恢复平衡而产生的**排异反应**。三阶段对应三种物理现象，越依赖 qi 的人越惨，凡人反而安全。

```
[震兆 10s] → [灵压坍缩 15s] → [法则紊乱 15s] → [寂灭扩散 15s] → [余震]
     ↓              ↓                 ↓                 ↓              ↓
 zone 预警     qi 真空吸出      规则崩溃自伤      死寂圈扩散      确认存亡
 天象骤暗      失压窒息感      自己的招式反噬     存在性威胁      地形永伤
```

### §2.1 震兆（10 秒）

绝壁劫不给 60s 预兆——它是"反震"不是"考试"，来得急且猛。

- zone 广播："天地一怒。" + 坐标
- 天象：整个 zone 天色骤暗，不是雷云——是**天光消失**（光照等级骤降至 4）
- 地面开始出现裂隙（§5 地形 VFX 开始执行）
- HUD：全 zone 玩家顶栏红幅 "绝壁劫 · 距震心 XXX 格"
- 10s 内可跑（但 zone 级范围，大部分人跑不出去）
- 触发者本人：**不可逃离**

### §2.2 第一相：灵压坍缩（15 秒）

**物理本质**：极端操作在震心制造了巨大的 qi 真空——周围灵气被猛烈吸入填补，连修士体内的真元也被"拽出来"。

**qi_physics 实现**：

```rust
/// 灵压坍缩期间，给半径内所有 entity 挂此 component。
#[derive(Component)]
pub struct JueBiPressureCollapse {
    pub epicenter: BlockPos,
    pub phase_start_tick: u64,
    pub distance: f64,
}

/// 每 tick 对受影响 entity 执行：强制 qi 外泄。
fn juebi_pressure_collapse_system(
    mut query: Query<(&JueBiPressureCollapse, &mut Cultivation)>,
    zone_registry: ResMut<ZoneRegistry>,
) {
    for (collapse, mut cult) in &mut query {
        // 距离衰减：50 格内全额，50-150 线性衰减，150+ 无影响
        let factor = collapse_factor(collapse.distance);

        // 强制 qi 外泄：每 tick 损失 qi_current 的 2% × factor
        // 不走 excretion 公式——这是"失压"，不是正常逸散
        let drain = cult.qi_current * 0.02 * factor;
        cult.qi_current = (cult.qi_current - drain).max(0.0);

        // 丹药/灵器/法阵内封存的 qi 也被抽出（container seal 无效化）
        // → 通知 shelflife 系统：这些物品的 seal_multiplier 临时变 1.0
    }

    // zone.spirit_qi 强制归零（整 zone 被吸空）
    // → 所有 regen_from_zone 调用返回 0
    // → 所有 qi_excretion_loss 按 local_zone_qi=0 计算（逸散最大化）
}

fn collapse_factor(distance: f64) -> f64 {
    if distance <= 50.0 {
        1.0
    } else if distance <= 150.0 {
        1.0 - (distance - 50.0) / 100.0
    } else {
        0.0
    }
}
```

**玩家体感**：
- 真元在不受控地流失（像舱体失压，呼吸都在漏气）
- 丹药/灵器失灵（封存的 qi 被拽出）
- 法阵断电（阵眼 qi 被吸空 → 阵法朽坏加速 ×10）
- 低境界修士（qi 少）流失少，反而更安全；化虚者池子大但流失比例相同 → 绝对损失巨大
- **地形响应**：地下灵脉被抽空 → 地表塌陷 → 裂隙形成（§5.3 算法因果）

### §2.3 第二相：法则紊乱（15 秒）

**物理本质**：灵压坍缩后 zone 灵气场不再是均匀的，局部出现混沌态——qi_physics 规则本身短暂崩溃。修士的技法在"错误的物理规则"下执行 = 反噬自己。

**qi_physics 实现**：

```rust
/// 法则紊乱期间挂的 component。
#[derive(Component)]
pub struct JueBiLawDisruption {
    pub epicenter: BlockPos,
    pub distance: f64,
    pub seed: u64,  // 决定紊乱方向（每个 entity 不同）
}

/// 扩展 EnvField，新增 law_disruption 字段。
impl EnvField {
    pub fn with_law_disruption(mut self, intensity: f64) -> Self {
        self.law_disruption = intensity.clamp(0.0, 1.0);
        self
    }
}

/// 法则紊乱对各 qi_physics 子系统的影响：
///
/// collision.rs — 攻击方向反转概率
///   if rng(seed) < law_disruption * 0.4 → damage 反作用到 attacker
///   修士出招有 40% 概率打自己（"法则紊乱，你的真元不认得路了"）
///
/// channeling.rs — 经脉流向不可控
///   flow_rate 随机 ×0.2~×3.0，过载概率飙升
///   本来安全的操作可能突然 overload → 经脉损伤
///
/// excretion.rs — 逸散方向反转
///   概率性 "吸入" 周围脏 qi 而非排出自身 qi
///   → 被污染（对毒蛊师有利、对洁净修士致命）
///
/// distance.rs — 距离计算偏移
///   实际 hit 距离 = 真实距离 × random(0.5, 2.0)
///   → 暗器射偏、截脉摸空、阵法范围失真

fn build_env_for_entity(
    entity: Entity,
    zone: &Zone,
    disruption: Option<&JueBiLawDisruption>,
    vortex: Option<&VortexFieldAffected>,
) -> EnvField {
    let mut env = EnvField::new(zone.spirit_qi);

    if let Some(d) = disruption {
        let intensity = disruption_factor(d.distance); // 50 格内 1.0 → 150 格 0.0
        env = env.with_law_disruption(intensity);
    }
    if let Some(v) = vortex {
        env = env.with_turbulence(v.intensity as f64);
    }
    env
}
```

**玩家体感**：
- 出招可能反噬自己（"你一拳打出去——真元回来了，打在自己经脉上"）
- 经脉流量不可控（想运气提防却突然过载撕裂）
- 距离感扭曲（明明对方在 10 格外，你的暗器飞了 20 格还没到）
- **越强越危险**：化虚者的招式威力大 → 反噬也大；凡人不用 qi → 不受影响
- **地形响应**：地壳力学紊乱 → 板块异动 → 锥山冲天（§5.4 算法因果）

### §2.4 第三相：寂灭扩散（15 秒）

**物理本质**：天地为彻底"愈合伤口"，从震心向外推出一圈"qi 归零场"——寂灭圈经过之处，所有 qi 活动停止。对化虚者（肉身已被 qi 重铸，维持形态依赖 qi）= 存在性威胁。

**qi_physics 实现**：

```rust
/// 寂灭场：从震心向外扩散的"死寂圈"。
#[derive(Resource)]
pub struct JueBiNullField {
    pub epicenter: BlockPos,
    pub current_radius: f64,     // 每 tick 扩大
    pub expansion_rate: f64,     // 10 blocks/tick = 15s 内扩到 150 格
    pub max_radius: f64,         // 150.0
    pub started_tick: u64,
}

/// 寂灭场内的 entity 挂此 component：所有 qi_physics 调用返回零。
#[derive(Component)]
pub struct JueBiNullified {
    pub entered_tick: u64,
    pub accumulated_null_time: f64,  // 累计在寂灭场内的时间
}

/// 寂灭场的致死机制（非 damage，是 qi 维持肉身失败）：
///
/// 化虚者：qi_current 每 tick -3%（肉身"记忆"在消散）
///   → qi_current = 0 时触发 "凡躯崩解" death
///   → 15s 内从满血到死需要 qi_current 耗尽 ≈ 可能撑住也可能不行
///
/// 通灵者：qi_current 每 tick -1%（肉身改造程度较浅）
///   → 不太会死，但出来后真元几乎归零
///
/// 固元及以下：qi_current 冻结（不涨不跌）
///   → 安全，只是不能修炼/恢复。凡人完全无事
///
/// **这就是"一损俱损"的物理依据**：
///   低境围剿化虚 → 化虚出绝招 → 寂灭场扩散 →
///   化虚者自己也在寂灭场里（大概率崩解死）→
///   但低境者几乎不受影响（他们不依赖 qi 维持肉身）→
///   真正被"俱损"的是**其他高境者**（通灵/化虚）
fn juebi_null_field_tick(
    mut null_field: ResMut<JueBiNullField>,
    mut commands: Commands,
    positions: Query<(Entity, &Position, Option<&Cultivation>)>,
    mut nullified: Query<(&mut JueBiNullified, &Cultivation)>,
    tick: Res<CurrentTick>,
) {
    // 扩大半径
    null_field.current_radius =
        (null_field.current_radius + null_field.expansion_rate)
            .min(null_field.max_radius);

    // 给新进入寂灭圈的 entity 挂 component
    for (entity, pos, _cult) in &positions {
        let dist = pos.distance_to(null_field.epicenter);
        if dist <= null_field.current_radius {
            commands.entity(entity).insert(JueBiNullified {
                entered_tick: tick.0,
                accumulated_null_time: 0.0,
            });
        }
    }

    // 寂灭场内 qi 衰减
    for (mut nulled, cult) in &mut nullified {
        nulled.accumulated_null_time += 1.0;
        let decay_rate = match cult.realm {
            Realm::Void => 0.03,      // 化虚：3%/tick → ~33 ticks 耗尽
            Realm::Spirit => 0.01,    // 通灵：1%/tick → 安全但虚弱
            _ => 0.0,                 // 固元及以下：不受影响
        };
        // qi_current 衰减由调用方处理（此处只标记 rate）
    }
}
```

**玩家体感**：
- 看到一圈"虚无"从震心向外推进（视觉：地面植被瞬间枯死变灰、水体蒸发、粒子消失）
- 被波及后：所有 qi 技能无法使用、真元在持续流失、灵器变铁块
- 化虚者：感到肉身在"散架"（worldview §三:187 凡躯重铸的逆过程）
- 低境者：只是暂时不能修炼，15s 后寂灭场消失就恢复了
- **地形响应**：寂灭场经过之处灵根植被枯死 → 土壤松散 → 地表翻涌（§5.5 算法因果）

### §2.5 余震

- 寂灭场扩散完毕后逐渐收缩（30s 内从 150 格缩回 0）
- zone.spirit_qi 从 0 慢慢恢复（5 分钟回到原值的 50%，30 天内完全恢复）
- 存活者 debuff "劫余"：24h 内 `rhythm_multiplier × 0.5`（恢复减半）
- 触发者若存活：生平卷记 "绝壁劫·存" + 24h 无法再次触发极端操作
- 触发者若死亡：走正常 death-lifecycle + 生平卷记 "绝壁劫·殁"
- **地形锥山/裂隙保留**（根据 §8 #3 决定临时还是永久）

---

## §3 存活条件

绝壁劫**理论上可以活**——关键在于你能否在 45 秒内撑过三相：

| 相 | 致死机制 | 化虚者存活条件 |
|----|---------|--------------|
| 灵压坍缩 | qi 外泄至 0 → 无力抵抗后续相 | qi_max 足够大 + 提前满真元（10700 × 0.98^300 ≈ 存活阈值） |
| 法则紊乱 | 反噬自伤（40% 概率自打） | **不出招**——紊乱期间主动使用 qi 技能才会反噬，站着不动反而安全 |
| 寂灭扩散 | qi_current 每 tick -3% → 归零 → 凡躯崩解 | 进场前 qi_current 还剩多少？前两相保存得越多越安全 |

| 存活路径 | 原理 | 预估存活率 |
|----------|------|-----------|
| 硬扛（满状态 + 不出招） | 坍缩期保真元 + 紊乱期不动 + 寂灭期硬撑 33 ticks | ~8% |
| 护龛阵庇护 | 护龛阵内 qi 封存不受坍缩吸出 → 进寂灭场时真元更满 | +12% |
| 替尸伪皮 | 伪皮在寂灭场内代替肉身承受"崩解"（伪皮碎，本体存活） | +15% |
| 300 格外（非触发者） | 三相均有距离衰减，150 格外坍缩 =0 / 紊乱 =0 / 寂灭场扩不到 | 100%（逃出去了） |
| 低境界 | 固元及以下寂灭场不衰减 qi → 只有坍缩期小损失 | ~90%（几乎安全） |

**核心设计**：
- **越强越危险**（化虚 > 通灵 > 固元 > 凡人）→ 天道"排异"的就是 qi 浓度高的存在
- **不动比乱动安全**（紊乱期反噬只在主动施法时触发）→ 鼓励"忍耐"而非"对抗"
- 存活率 <10% 但非零 → 绝壁劫是"孤注一掷的终极赌博"，不是自杀按钮

---

## §4 一损俱损机制（核心社会威慑）

绝壁劫的威慑不是"低境必死"——**低境反而安全**。真正的"一损俱损"是：

**围剿化虚者的人如果自身也是高境（通灵/化虚），绝壁劫会同时杀死他们。**

| 围剿者境界 | 绝壁劫三相对其影响 | 后果 |
|-----------|-------------------|------|
| 化虚 | 坍缩全吃 + 紊乱全吃 + 寂灭 3%/tick | **极可能一起死** |
| 通灵 | 坍缩吃 + 紊乱吃 + 寂灭 1%/tick | 重伤，真元归零 |
| 固元/凝脉 | 坍缩少吃 + 紊乱不动就不吃 + 寂灭不影响 | **几乎安全** |
| 凡人 | 全不影响 | 安全（看个热闹） |

**社会效果**：
- **化虚 vs 化虚**：双方都不想逼对方出绝招——绝壁劫对自己伤害一样大
- **通灵围剿化虚**：你得考虑清楚，逼急了化虚出绝招你自己也残
- **低境围剿化虚**：其实 OK——低境者不受寂灭场影响，但你怎么打得过化虚？
- 结果："化虚者 = 核弹"的认知不是对所有人的，是**对高境者**的。化虚之间和平靠威慑，不靠道德

**反制路径**（不让化虚无敌）：
- **远程消耗战**（150 格外三相衰减为 0）→ 暗器流射程优势
- **信息战**（提前定位 → 不靠近就不触发）→ 毒蛊流暴露系统价值
- **欺天阵嫁祸**（诱导化虚对假目标出绝招 → 绝壁劫落在无人区）→ 阵法流价值
- **截脉封经**（SEVERED 关键经脉 → 无法 cast 绝招 → 不触发）→ 截脉流价值
- **低境群殴**（固元以下组队围剿 → 对寂灭场免疫 → 但战力差距巨大需要人海）→ 末法世界"以弱胜强"的社会路径

---

## §5 地形反震 VFX（方块级视觉）

绝壁劫不只是粒子效果——**真实修改地形方块**造成视觉冲击：地面开裂、石柱冲天、地表翻涌。

### §5.1 技术接入点

| 层 | 接口 | 用途 |
|----|------|------|
| 方块写入 | `ChunkLayer.set_block(BlockPos, BlockState)` | Valence 自动 batch 为 `MultiBlockChangeS2c` |
| 原始保存 | `JueBiTerrainOverlay` resource（同 `TribulationOmenCloudBlocks` 模式） | save → place → restore |
| 分帧预算 | 每 tick ≤200 blocks（~8ms），通过 ring-buffer 队列摊开 | 避免卡帧 |
| 持久化 | `JueBiScarRegistry`（同 `SpiritWoodHarvestedLogs`）| 若 §7 #3 选 A/C，chunk reload 时重播 |
| Chunk 边界 | `layer.chunk(ChunkPos).is_some()` 前置校验 | 未加载 chunk 不写 |

### §5.2 数据结构与动画原理

**核心原理**：`pending` 队列的**入队顺序即动画播放顺序**。每 tick 出队 200 blocks 执行 `set_block`，Valence 自动 batch 发包给客户端。只要入队时按"视觉展开方向"排列，玩家就能看到：

- 裂隙：从震心向外逐步"撕开"（按 `step` 距离入队）
- 锥山：从地面向上逐层"升起"（按 `dy` 高度入队）

**不需要额外动画系统**——方块修改本身就是动画帧，tick rate = 帧率。

```rust
#[derive(Resource, Default)]
pub struct JueBiTerrainOverlay {
    pending: VecDeque<TerrainModOp>,      // 按动画顺序排列的待执行队列
    placed: Vec<JueBiTerrainBlock>,       // 已放置（用于 restore）
    budget_per_tick: usize,               // 默认 200
}

struct TerrainModOp {
    pos: BlockPos,
    new_state: BlockState,
    anim_order: u32,  // 动画帧序号（越小越先执行）
}

struct JueBiTerrainBlock {
    pos: BlockPos,
    original: BlockState,
    placed_at_tick: u64,
    restore_at_tick: u64,    // 余震结束时 restore
    scar_permanent: bool,    // 是否写入 ScarRegistry 永久保留
}

/// 每 tick 执行：出队 budget 个 ops，写入 ChunkLayer
fn juebi_terrain_tick_system(
    mut overlay: ResMut<JueBiTerrainOverlay>,
    mut layer: Query<&mut ChunkLayer>,
    tick: Res<CurrentTick>,
) {
    let mut layer = layer.single_mut();
    let budget = overlay.budget_per_tick;

    for _ in 0..budget {
        let Some(op) = overlay.pending.pop_front() else { break };
        // 保存原始方块（用于后续 restore）
        if let Some(original) = layer.block(op.pos).map(|b| b.state()) {
            overlay.placed.push(JueBiTerrainBlock {
                pos: op.pos,
                original,
                placed_at_tick: tick.0,
                restore_at_tick: tick.0 + AFTERSHOCK_DURATION_TICKS,
                scar_permanent: false, // §8 #3 决定
            });
        }
        layer.set_block(op.pos, op.new_state);
    }
}
```

### §5.3 算法一：径向裂隙（Radial Fissure）

从震心向外辐射 N 条裂缝，每条是带随机偏转的射线，沿途向下挖深。

```rust
/// 生成所有径向裂隙，结果追加到 ops 队列。
fn generate_radial_fissures(
    ops: &mut VecDeque<TerrainModOp>,
    rng: &mut impl Rng,
    epicenter: BlockPos,
    num_cracks: usize,  // 8
    radius: f64,        // 80.0
) {
    let angle_step = std::f64::consts::TAU / num_cracks as f64;

    for i in 0..num_cracks {
        let base_angle = angle_step * i as f64 + rng.gen_range(-0.3..0.3);
        let length = radius * rng.gen_range(0.6..1.0);
        trace_crack(ops, rng, epicenter, base_angle, length);
    }
}

/// 单条裂隙：沿 angle 步进，每步随机偏转 ±15°（random walk）。
///
/// **动画关键**：blocks 按 step（距震心距离）顺序入队。
/// 每 tick 出队 200 个 → 玩家看到裂缝从震心向外"一格格撕开"。
/// 8 条裂隙的 blocks 按 step 交错合并（见 merge_by_anim_order），
/// 使 8 条裂隙同时向外蔓延，而非一条跑完再跑下一条。
fn trace_crack(
    ops: &mut Vec<(u32, TerrainModOp)>,  // (anim_order, op) — 后续按 order 排序
    rng: &mut impl Rng,
    origin: BlockPos,
    initial_angle: f64,
    length: f64,
) {
    let mut x = origin.x as f64;
    let mut z = origin.z as f64;
    let mut angle = initial_angle;

    for step in 0..length as usize {
        x += angle.cos();
        z += angle.sin();
        angle += rng.gen_range(-0.26..0.26); // ±15° random walk

        let ratio = step as f64 / length;
        let depth = crack_depth(ratio, rng);
        let width = crack_width(ratio);

        let perp_sin = angle.sin();
        let perp_cos = angle.cos();

        for dw in -(width / 2)..=(width / 2) {
            let wx = (x + dw as f64 * perp_sin).round() as i32;
            let wz = (z - dw as f64 * perp_cos).round() as i32;
            let surface_y = find_surface_y(wx, wz);

            for dy in 0..depth {
                let pos = BlockPos::new(wx, surface_y - dy, wz);
                let block = if dy >= depth - 1 {
                    BlockState::MAGMA_BLOCK
                } else if dy >= depth - 3 {
                    BlockState::DEEPSLATE
                } else {
                    BlockState::AIR
                };
                // anim_order = step → 同一 step 的所有 blocks 同帧出队
                ops.push((step as u32, TerrainModOp { pos, new_state: block, anim_order: step as u32 }));
            }
        }
    }
}

/// 合并多条裂隙的 blocks，按 anim_order 排序后推入 pending 队列。
/// 效果：8 条裂隙同时向外推进（step 0 的 8 条先出，step 1 的 8 条再出...）
fn merge_by_anim_order(
    pending: &mut VecDeque<TerrainModOp>,
    mut all_ops: Vec<(u32, TerrainModOp)>,
) {
    all_ops.sort_by_key(|(order, _)| *order);
    for (_, op) in all_ops {
        pending.push_back(op);
    }
}

/// 裂隙深度：震心最深 ~50 格，边缘浅。
/// 与锥山高度（50-100 格）形成 ~100 格高度差。
fn crack_depth(ratio: f64, rng: &mut impl Rng) -> i32 {
    match ratio {
        r if r < 0.2 => rng.gen_range(40..=50), // 深裂（震心附近，底部岩浆）
        r if r < 0.5 => rng.gen_range(20..=40), // 中裂
        r if r < 0.8 => rng.gen_range(8..=20),  // 浅裂
        _            => rng.gen_range(2..=8),    // 地表纹路
    }
}

/// 裂隙宽度：震心 5 格 → 远端 1 格。
fn crack_width(ratio: f64) -> i32 {
    match ratio {
        r if r < 0.2 => 5,
        r if r < 0.5 => 3,
        r if r < 0.8 => 2,
        _            => 1,
    }
}
```

**参数配置（绝壁劫规格）**：
- `num_cracks = 8`（8 条主裂隙）
- `radius = 80`（核心区 50 格内必有裂隙经过）
- 总方块修改量：~8 × 80 × (avg_width 3) × (avg_depth 25) ≈ **48,000 blocks**
- 按 200 blocks/tick：~240 ticks = **12 秒**（震兆期开始，延续到天降期结束）
- **深度**：震心附近 40-50 格深，底部岩浆发光可见

### §5.4 算法二：冲天锥山（Eruption Cones）

震心附近 50 格内随机位置冲出 5-8 座圆锥形山体，底部半径 8-15 格，高度 20-50 格。

```rust
/// 生成一座圆锥形冲天山。
/// 从地表向上逐层收缩：底部 base_radius → 顶部 1-2 格尖顶。
fn generate_eruption_cone(
    ops: &mut VecDeque<TerrainModOp>,
    rng: &mut impl Rng,
    epicenter: BlockPos,
    max_spawn_radius: i32,  // 锥山生成位置距震心最远距离
) {
    // 位置：偏向震心（平方根反分布）
    let r = max_spawn_radius as f64 * (1.0 - rng.gen::<f64>().sqrt()) * 0.8 + 8.0;
    let theta = rng.gen::<f64>() * std::f64::consts::TAU;
    let cx = epicenter.x + (r * theta.cos()) as i32;
    let cz = epicenter.z + (r * theta.sin()) as i32;

    // 锥体参数：离震心越近越高越宽
    // 高度 50-100 格，与裂隙深度 40-50 格形成 ~100 格高度差
    let dist_ratio = r / max_spawn_radius as f64;
    let height = lerp(100.0, 50.0, dist_ratio) as i32 + rng.gen_range(-8..=8);
    let base_radius = lerp(18.0, 10.0, dist_ratio) as i32 + rng.gen_range(-2..=2);

    let surface_y = find_surface_y(cx, cz);

    // 逐层构建圆锥截面
    // **动画关键**：按 dy（高度）顺序入队。
    // 每 tick 出队 200 blocks → 玩家看到锥山从地面一层层"升起"。
    // 多座锥山的 blocks 按 dy 交错合并，使它们同时生长。
    for dy in 0..height {
        let layer_ratio = dy as f64 / height as f64;

        // 圆锥收缩：线性从 base_radius → 1（顶部尖峰）
        let layer_radius = base_radius as f64 * (1.0 - layer_ratio * 0.93);
        // 噪声扰动边缘（避免完美圆形）
        let noise_amp = layer_radius * 0.25;

        let r_ceil = layer_radius.ceil() as i32 + 1;
        for dx in -r_ceil..=r_ceil {
            for dz in -r_ceil..=r_ceil {
                let dist = ((dx * dx + dz * dz) as f64).sqrt();
                let noise = perlin_3d(
                    (cx + dx) as f64 * 0.15,
                    dy as f64 * 0.2,
                    (cz + dz) as f64 * 0.15,
                ) * noise_amp;

                if dist <= layer_radius + noise {
                    let pos = BlockPos::new(cx + dx, surface_y + dy, cz + dz);
                    let block = cone_block(dy, height, dist, layer_radius, rng);
                    ops.push_back(TerrainModOp {
                        pos,
                        new_state: block,
                        anim_order: dy as u32,  // dy = 动画帧序号
                    });
                }
            }
        }
    }
}

/// 锥山方块材质分层
fn cone_block(
    dy: i32,
    total_height: i32,
    dist_from_axis: f64,
    layer_radius: f64,
    rng: &mut impl Rng,
) -> BlockState {
    let height_ratio = dy as f64 / total_height as f64;
    let edge_ratio = dist_from_axis / layer_radius.max(1.0);

    // 外壳 vs 内核
    let is_shell = edge_ratio > 0.75;

    match (height_ratio, is_shell) {
        // 底部 25%：深板岩内核 + 深板岩圆石外壳
        (r, _) if r < 0.25 => {
            if is_shell { BlockState::COBBLED_DEEPSLATE } else { BlockState::DEEPSLATE }
        }
        // 中段 25-65%：玄武岩主体 + 随机裂纹
        (r, _) if r < 0.65 => {
            if rng.gen::<f64>() < 0.12 {
                BlockState::CRYING_OBSIDIAN  // 12% 紫光裂纹（灵气残留）
            } else if is_shell {
                BlockState::POLISHED_BASALT
            } else {
                BlockState::BASALT
            }
        }
        // 顶部 65-90%：黑石 + 更密集的 crying obsidian
        (r, _) if r < 0.90 => {
            if rng.gen::<f64>() < 0.20 {
                BlockState::CRYING_OBSIDIAN
            } else {
                BlockState::BLACKSTONE
            }
        }
        // 尖顶 90-100%：obsidian 实心
        _ => BlockState::OBSIDIAN,
    }
}

/// 批量生成 count 座锥山
fn generate_all_cones(
    ops: &mut VecDeque<TerrainModOp>,
    rng: &mut impl Rng,
    epicenter: BlockPos,
    count: usize,       // 5-8 座
    max_radius: i32,    // 50 格
) {
    for _ in 0..count {
        generate_eruption_cone(ops, rng, epicenter, max_radius);
    }
}
```

**参数配置**：
- `count = 6`（6 座锥山）
- `max_radius = 50`（核心区内）
- 每锥平均 height=75, base_radius=14 → 截面面积 ~π×14² ≈ 615 blocks/层 × 75 层 × fill_ratio ~0.45 ≈ **20,756 blocks/锥**
- 总方块量：6 × 20,756 ≈ **124,536 blocks**
- 按 200 blocks/tick：~623 ticks = **31 秒**（天降 5s 开始，延续到劫波全程——视觉上锥山在战斗中仍在"生长"，极具压迫感）
- **高度差**：锥山顶 +100 格 vs 裂隙底 -50 格 = 最大落差 **150 格**（玩家站裂隙边缘仰望锥山 = 末日感）

### §5.5 算法三：地表翻涌（Surface Upheaval）

重伤区（50-150 格）地面块被随机抬升/下沉 1-5 格，制造"大地震动"效果。

```rust
/// 地表翻涌：环带内以 density 概率对地表列做垂直位移。
fn generate_surface_upheaval(
    ops: &mut VecDeque<TerrainModOp>,
    layer: &ChunkLayer,
    epicenter: BlockPos,
    inner_r: i32,   // 50
    outer_r: i32,   // 120
    density: f64,   // 0.35
    rng: &mut impl Rng,
) {
    let noise_seed = rng.gen::<u32>();

    for x in (epicenter.x - outer_r)..=(epicenter.x + outer_r) {
        for z in (epicenter.z - outer_r)..=(epicenter.z + outer_r) {
            let dx = (x - epicenter.x) as f64;
            let dz = (z - epicenter.z) as f64;
            let dist = (dx * dx + dz * dz).sqrt();

            if dist < inner_r as f64 || dist > outer_r as f64 {
                continue;
            }
            // 密度筛选（hash 代替 per-column rng，保证确定性）
            if hash_density(x, z, noise_seed) > density {
                continue;
            }

            // 位移量：离震心越近越剧烈
            let dist_ratio = (dist - inner_r as f64) / (outer_r - inner_r) as f64;
            let max_shift = lerp(5.0, 1.0, dist_ratio);
            let noise = simplex_2d(x as f64 * 0.05, z as f64 * 0.05, noise_seed);
            let shift = (noise * max_shift).round() as i32;

            if shift == 0 {
                continue;
            }

            let surface_y = find_surface_y(x, z);

            if shift > 0 {
                // 抬升：复制地表材质向上堆叠
                let surface_block = layer
                    .block(BlockPos::new(x, surface_y, z))
                    .map(|b| b.state())
                    .unwrap_or(BlockState::STONE);

                for dy in 1..=shift {
                    ops.push_back(TerrainModOp {
                        pos: BlockPos::new(x, surface_y + dy, z),
                        new_state: surface_block,
                        priority: 2,
                    });
                }
            } else {
                // 下沉：移除地表层
                for dy in 0..shift.unsigned_abs() as i32 {
                    ops.push_back(TerrainModOp {
                        pos: BlockPos::new(x, surface_y - dy, z),
                        new_state: BlockState::AIR,
                        priority: 2,
                    });
                }
            }
        }
    }
}
```

**参数配置**：
- `inner_r = 50, outer_r = 120`（核心区被裂隙+锥山占了，翻涌填补重伤区）
- `density = 0.35`
- 环带面积 ≈ π×(120²-50²) ≈ 37,385 列 × 0.35 × avg_shift 2.5 ≈ **32,712 blocks**
- 按 200 blocks/tick：~164 ticks = **8.2 秒**（劫波期间持续铺开）

### §5.6 执行时序

```
震兆 10s (200 ticks) ─────────────────────────────
  tick 0-200:   径向裂隙由近及远蔓延（~48k blocks, 优先级 0 先出队）
                ↳ 地面"撕开"，50 格深裂缝底部岩浆发光
  tick 100+:    裂隙延续到天降期（深度 + 宽度还在扩）

天降 5s (100 ticks) ──────────────────────────────
  tick 0-100+:  锥山从裂隙间冲出（~125k blocks, 逐层构建 = 视觉上"生长"）
                ↳ 生长持续整个劫波期（总 ~623 ticks 跨阶段）
                ↳ 高度 50-100 格 — 远处玩家能看到天际线被改变
  tick 0-20:    锁定圈边界黑曜石墙升起

劫波 30-45s (600-900 ticks) ──────────────────────
  每波开始:     地表翻涌触发（~33k blocks, 优先级 2 与锥山交替出队）
  波间 5s:      碎石粒子 + 小裂隙支线补充
  全程:         锥山仍在"生长"直到 100 格尖顶完成 → 视觉高潮
                ↳ 玩家在裂隙（-50格）和锥山（+100格）之间战斗 = 150 格落差战场

余震 300s ────────────────────────────────────────
  tick 0-300:   裂隙底部岩浆逐渐冷却（MAGMA → DEEPSLATE，100 blocks/tick）
  tick 300-500: 锥山顶部 CRYING_OBSIDIAN → BLACKSTONE（紫光熄灭）
  余震结束:     根据 §8 #3 决定 restore 还是 persist
```

**总方块修改量**：~48k（裂隙）+ ~125k（锥山）+ ~33k（翻涌）≈ **206,000 blocks**
按 200 blocks/tick 全程 ~1,030 ticks = **51 秒**，分布在震兆→天降→劫波全三阶段。玩家视觉上：地面先裂开（深渊），然后石山从裂隙间冲天而起（高低差 150 格），最后周围地表翻涌——整个 zone 地形面目全非。

### §5.7 方块调色板

| 元素 | 方块 | 意图 |
|------|------|------|
| 裂隙空洞 | `AIR` | 深裂 |
| 裂隙底 | `MAGMA_BLOCK`（发光） | 地底岩浆外露 |
| 裂隙壁 | `DEEPSLATE` | 深层暴露 |
| 石柱底 | `DEEPSLATE` | 从地底冲上来的质感 |
| 石柱身 | `BASALT` / `POLISHED_BASALT` | 玄武岩柱（六棱柱自然形态） |
| 石柱顶 | `BLACKSTONE` + `CRYING_OBSIDIAN`(15%) | 暗色 + 紫色泪光 = 灵气残留 |
| 翻涌 | 原地表块 / `AIR` | 保持地形材质连续性 |
| 锁定圈边界 | `OBSIDIAN`（临时墙） | 黑色硬边界，视觉压迫感 |

### §5.8 恢复策略

两种模式（取决于 §7 #3 决策）：

**临时模式**（余震结束后 restore）：
- `JueBiTerrainOverlay.placed` 全部按 original 恢复
- 恢复也分帧（200 blocks/tick），从外到内恢复（视觉上像大地愈合）

**伤痕模式**（永久保留）：
- 写入 `JueBiScarRegistry` resource
- 在 `ensure_chunk_generated` 流程末尾重播（同 `SpiritWoodHarvestedLogs` 模式）
- 伤痕上生长灰色植被（30 天后逐渐恢复 → 触发 restore）

---

## §6 void-quota 超额改造

当前 `plan-void-quota-v1` 实装：超额起劫 → `VoidQuotaExceeded` → 瞬死。

v2 改为：超额起劫 → 渡虚劫正常进行 → **结算时触发绝壁劫**（渡虚劫最后一波变绝壁劫波）：
- 渡虚劫正常 3-5 波后，如果超额 → 不是 Ascended，而是天道"多送几波"
- 相当于：你扛过了渡虚劫所有波次，但天道不认你 → 追加绝壁劫 2-3 波
- 观战者 / 截胡者也被波及
- 如果还能活（<5%）→ 天道承认你 → 强行挤掉最弱化虚者的名额（另一个化虚者强制退境）

设计理由：这比"瞬死"更有叙事性 + 保留极小希望 + "一损俱损"波及截胡者。

---

## §7 技术实装方向

### P0 — 绝壁劫 Kind + 天地排异三相 + 地形反震（5 周）

- [ ] `TribulationKind::JueBi` 新变体
- [ ] `JueBiTriggerEvent` 通用触发接口（各 v2 plan emit 此事件）
- [ ] 绝壁劫阶段流程：震兆(10s) → 灵压坍缩(15s) → 法则紊乱(15s) → 寂灭扩散(15s) → 余震
- [ ] `JueBiPressureCollapse` component + 距离衰减 qi 外泄 system
- [ ] `JueBiLawDisruption` component + `EnvField.law_disruption` 字段扩展
  - [ ] collision.rs 反噬概率（`law_disruption × 0.4`）
  - [ ] channeling.rs 流量随机化（`×0.2~×3.0`）
  - [ ] distance.rs 命中距离偏移（`×0.5~×2.0`）
- [ ] `JueBiNullField` resource + 扩散系统 + 境界分级衰减
  - [ ] 化虚 3%/tick / 通灵 1%/tick / 固元以下 0%
  - [ ] qi_current 归零 → 凡躯崩解 death cause
- [ ] `JueBiTerrainOverlay` resource + 分帧队列（200 blocks/tick budget）
- [ ] 径向裂隙算法实装（§5.3：8 条主裂隙，80 格半径，random walk 偏转）
- [ ] 冲天锥山算法实装（§5.4：6 座锥山，50 格内，高 50-100 格）
- [ ] 地表翻涌算法实装（§5.5：50-120 格环带，simplex noise 位移）
- [ ] 余震期恢复/持久化逻辑（§5.8：按 §8 #3 决策选模式）
- [ ] 存活/死亡结算（BiographyEntry 记录）
- [ ] 余震 debuff（24h `rhythm_multiplier × 0.5`）
- [ ] 测试：三相各自独立 + 联合链路 + 距离衰减 + 境界分级 + 地形动画顺序 + 存活/死亡分支 ≥50 单测

### P1 — void-quota 超额改造 + 各 v2 plan 触发接入（2 周）

- [ ] 改 `check_void_quota` 超额路径：渡虚劫结算时 emit `JueBiTriggerEvent` 而非瞬死
- [ ] 接入 dugu-v2 `cast_reverse`（30s 延迟触发）
- [ ] 接入 baomai-v3 `cast_disperse`（连续 3 次触发）
- [ ] 接入 woliu-v2 `vortex_heart`（30s 延迟触发）
- [ ] 接入 zhenfa-v2 欺天阵识破反噬（10s 触发）
- [ ] `KarmaWeightStore` → 绝壁劫强度修正（天道注视累积越高 → 绝壁劫越强）
- [ ] 测试：各触发源 + 延迟 + 强度修正 ≥20 单测

### P2 — 客户端 VFX / HUD / 音效（3 周）

- [ ] 绝壁劫天象 VFX（zone 级暗天 + 四方向雷云压迫 + 地裂粒子）
- [ ] 裂隙粒子（裂缝边缘 LAVA_DRIP + SMOKE 向上冒）
- [ ] 石柱冲天粒子（底部 EXPLOSION + 碎石 BLOCK_CRACK 飞散）
- [ ] 地面震动 screen-shake（距离震心越近越强）
- [ ] HUD 顶栏红幅（zone 广播 + 距离显示 + 倒计时）
- [ ] 核心区/重伤区/波及区视觉边界（黑曜石墙 + 粒子环）
- [ ] 4 音效 recipe（ground_crack_rumble / pillar_eruption_boom / pressure_collapse_whoosh / aftershock_wind）
- [ ] 余震 debuff HUD 图标
- [ ] 测试：VFX 回归 + HUD 集成 + screen-shake 距离衰减

### P3 — agent narration + 社会效果（2 周）

- [ ] `tiandao::juebi_runtime`（绝壁劫专属 narration template）
  - 震兆："天地一怒。"（zone 广播）
  - 劫中："灵压崩塌。修士藏器、药草、法阵——都在碎。"
  - 存活："这次反震之后还活着的……天道记住你了。"
  - 死亡："天地不容。"
- [ ] 江湖传闻（"XX 处发生绝壁劫，方圆百里灵气断绝"）
- [ ] 化虚者使用绝招的政治博弈叙事（"逼化虚出手 = 全村陪葬"）
- [ ] 测试：narration 古意检测 + 语义覆盖

### P4 — 平衡回归 + 饱和测试（2 周）

- [ ] 强度曲线 telemetry（各触发源存活率 / AOE 实际覆盖 / zone 灵气恢复时间）
- [ ] 反制路径验证（300 格外安全 / 护龛阵 / 替尸伪皮）
- [ ] 与 plan-style-balance-v1 矩阵对齐
- [ ] 饱和测试 audit：绝壁劫全链路 ≥60 单测
- [ ] e2e：client 触发 → server 绝壁劫序列 → zone AOE → 存活/死亡 → client 渲染
- [ ] Finish Evidence + 迁入

---

## §8 开放问题 / 决策门

### #1 绝壁劫 AOE 衰减模型

- **A**：线性衰减（50格 ×1.5 → 300格 ×0）
- **B**：三档阶梯（50/150/300 = ×1.5/×1.0/×0.5）
- **C**：物理衰减（×1.5 / distance²，跟 qi_physics 一致）

**默认推 B** —— 三档简洁 + 玩家容易理解"哪里安全"。C 虽物理正确但玩家难估算。

### #2 void-quota 超额后如果活下来——挤掉最弱化虚者？

- **A**：存活 → 强行挤掉最弱化虚（天道选择更强的那个）
- **B**：存活 → 半步化虚（保持 v1 行为，不挤人）
- **C**：存活 → 正式化虚，名额临时超 1（天道容忍 60 天内恢复）

**默认推 A** —— "天地装不下两个化虚"叙事 + 制造化虚间紧张关系。

### #3 绝壁劫是否写入 zone 永久记录（类似域崩）

- **A**：是（绝壁劫后 zone 变"伤痕地"，灵气恢复慢 30 天）
- **B**：否（5 分钟余震后完全恢复）
- **C**：看触发源强度（倒蚀/涡心 → 永久伤痕；散功 → 临时）

**默认推 C** —— 区分"zone 级破坏"和"个人极端操作"。

### #4 触发者 30s 延迟期间可否被击杀阻止绝壁劫

- **A**：可以（30s 内击杀触发者 → 绝壁劫取消 → 鼓励速杀）
- **B**：不可以（绝壁劫已触发不可逆 → 击杀无意义）
- **C**：击杀后绝壁劫降级为弱版（1 波 + ×0.5 强度）

**默认推 C** —— 给反制路径但不完全取消。如果 A 的话化虚者出绝招等于白给（30s 内被秒就没效果），削弱了"别逼化虚"的威慑。

### #5 绝壁劫期间其他人可否攻击触发者（类似截胡）

- **A**：可以（趁绝壁劫补刀 → 但自己也挨 zone AOE）
- **B**：不可以（绝壁劫期间天道保护触发者不受 PVP → 纯人 vs 天）

**默认推 A** —— 保持 v1 §2.5 精神：天道不偏袒任何一人。

---

## §9 进度日志

- **2026-05-10** 骨架立项。承接 plan-tribulation-v1 ✅ finished + plan-void-quota-v1 ✅ finished。核心：重定义绝壁劫为超强天劫（非必死）+ zone 级 AOE 一损俱损。目标：化虚 = 行走的核弹，别逼化虚出绝招。
  - 设计决策：绝壁劫是"物理反震"不是"惩罚"——化虚极端操作搅动天地灵气 → 天道自然降劫
  - void-quota 超额改造：从瞬死改为渡虚劫 + 追加绝壁劫波（保留极小存活率）
  - 社会威慑成立条件：zone 级 AOE + 距离衰减 + 不可阻止 = "围剿化虚者自己也要跑"
  - 前置依赖清晰：v1 全链路 + void-quota 已落，v2 只做绝壁劫扩展 + void-quota 改造
  - 反制路径：远程/信息/嫁祸/封经 四条路，确保化虚不是无敌

---

## Finish Evidence

### 落地清单

- P0 绝壁劫 server 运行时：`server/src/cultivation/tribulation.rs` 新增 `TribulationKind::JueBi`、`JueBiTriggerEvent` / `JueBiTriggeredEvent`、`PendingJueBiTriggers`、三相阶段机、`JueBiPressureCollapse`、`JueBiLawDisruption`、`JueBiNullField` / `JueBiNullified`、`JueBiTerrainOverlay`、zone aftershock 与 `JueBiAftershockDebuff`。
- P0 qi_physics 接入：`server/src/qi_physics/{env,collision,channeling,distance}.rs` 新增 `EnvField.law_disruption`，把反噬比例、运气倍率、距离偏移收口到既有 qi physics surface。
- P0/P2 视听与网络：`server/src/network/{tribulation_state_emit,tribulation_broadcast_emit,audio_trigger}.rs` 推送 `kind: "jue_bi"` 与 JueBi 触发/阶段事件；`server/assets/audio/recipes/{ground_crack_rumble,pillar_eruption_boom,pressure_collapse_whoosh,aftershock_wind}.json` 入默认音频表。
- P1 void-quota / 极端操作触发：`server/src/cultivation/void/actions.rs` 的 `ExplodeZone` 发 `JueBiTriggerEvent`；`server/src/combat/woliu_v2/tick.rs` 的长期 active `WoliuVortexHeart` 发延迟 JueBi；`check_void_quota` 超额不再瞬死，改为先走 DuXu，结算后追加 JueBi。
- P1 强度修正：`server/src/world/karma::KarmaWeightStore` 作为可选资源参与 `juebi_intensity_for_source()`；未来触发源统一走 `JueBiTriggerSource`。
- P2 client VFX / HUD surface：`client/src/main/java/com/bong/client/visual/particle/JueBiTribulationPlayer.java` 与 `VfxBootstrap.java` 注册 `bong:juebi_boundary` / `bong:juebi_fissure` / `bong:juebi_eruption`；已有 `tribulation_state` / `tribulation_broadcast` HUD store 接收 `jue_bi` kind。
- P3 agent narration / schema：`agent/packages/schema/src/{tribulation,biography}.ts` 与 generated schema 支持 `jue_bi`、`JueBiSurvived`、`JueBiKilled`；`agent/packages/tiandao/src/tribulation-runtime.ts` 与 `skills/tribulation.md` 补绝壁劫叙事。
- P4 回归覆盖：server 新增绝壁劫触发、void-quota 追加、三相 marker、地形动画顺序、余震 debuff、音频 recipe 计数等回归；agent/client 走既有契约与构建矩阵。

### 关键 commits

- `d8e4d37a4` · 2026-05-10 · `plan-tribulation-v2: 接入绝壁劫 server 运行时`
- `3dc492235` · 2026-05-10 · `plan-tribulation-v2: 接入绝壁劫 agent 契约`
- `16bd36b84` · 2026-05-10 · `plan-tribulation-v2: 接入绝壁劫 client VFX`

### 测试结果

- `cd server && cargo fmt --check` → passed。
- `cd server && cargo clippy --all-targets -- -D warnings` → passed。
- `cd server && cargo test` → 3581 passed。
- `cd server && cargo test juebi -- --nocapture` → 8 passed。
- `cd server && cargo test tribulation -- --nocapture` → 113 passed。
- `cd server && cargo test qi_physics -- --nocapture` → 100 passed。
- `cd server && cargo test woliu_v2 -- --nocapture` → 141 passed。
- `cd server && cargo test void_action -- --nocapture` → 17 passed。
- `cd server && cargo test audio -- --nocapture` → 15 passed。
- `cd server && cargo test network::audio_event_emit -- --nocapture` → 4 passed。
- `cd agent && npm run build` → passed。
- `cd agent/packages/schema && npm test` → 14 files / 351 tests passed。
- `cd agent/packages/tiandao && npm test` → 45 files / 320 tests passed。
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH" ./gradlew test build` → BUILD SUCCESSFUL。
- `git diff --check origin/main..HEAD` → clean。

### 跨仓库核验

- server：`TribulationKind::JueBi`、`JueBiTriggerEvent`、`JueBiTriggeredEvent`、`JueBiAfterDuXuQuota`、`JueBiAftershockDebuff`、`TribulationEventV1::jue_bi`、`bong:juebi_boundary` / `bong:juebi_fissure` / `bong:juebi_eruption`。
- agent schema / tiandao：`TribulationKindV1` 包含 `"jue_bi"`；`BiographyEntryV1` 包含 `JueBiSurvived` / `JueBiKilled`；`TribulationNarrationRuntime` 对 `kind === "jue_bi"` 输出天地反震叙事。
- client：`JueBiTribulationPlayer` 注册三条 JueBi VFX ID；既有 `TribulationStateHandler` / `TribulationBroadcastHandler` 保持通用 `jue_bi` 状态和广播显示。

### 遗留 / 后续

- `dugu-v2`、`baomai-v3`、`zhenfa-v2` 当前还不是本仓库可接入的已落地模块；本 plan 已预留 `JueBiTriggerSource::{DuguReverse,BaomaiDisperse,ZhenfaDeceptionExposed}` 与通用 `JueBiTriggerEvent`，后续对应 plan 只需 emit 事件，不再新增第二套绝壁劫引擎。
- 本次按 KISS/YAGNI 复用已有 tribulation HUD / broadcast / state 通道，没有单独新建 JueBi HUD store；专属表现集中在 server 地形 overlay、音效和 client VFX。
- 绝壁劫伤痕目前按 5 分钟恢复到原 qi 的 50% 并保留 `jue_bi_scar` active event；30 天世界级长期恢复可在后续 zone persistence / balance plan 中扩展。
