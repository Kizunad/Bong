//! 火候进程 / 会话（plan-alchemy-v1 §1.3）。
//!
//! `AlchemySession` 是炉体上挂载的运行时状态机。本模块提供：
//!   * 入炉（start_session）→ 扣材料（plan §1.3 投入即消耗）
//!   * 中途投料（submit_stage_materials）
//!   * 介入（Intervention：AdjustTemp / InjectQi）
//!   * tick 推进（每服务器 tick 一次）
//!   * 结束判定 → OutcomeBucket
//!
//! session 不依赖 ECS；外层 system 把 session 挂到炉实体并驱动它。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::incense::{ActiveIncense, IncenseBurn, IncenseEffect};
use super::outcome::{
    classify_precise, compute_duration_deviation, DeviationSummary, OutcomeBucket,
};
use super::recipe::{Recipe, RecipeId};
use super::skill_hook::tolerance_scale;

const MAX_INCENSE_BURNS: usize = 64;

/// 玩家介入事件（plan §1.3）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Intervention {
    AdjustTemp(f64),
    InjectQi(f64),
    /// 预留：未来绑定预设曲线（plan §1.3 自动化钩子）。
    AutoProfile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedMaterials {
    /// material → count（累积全阶段）
    pub materials: HashMap<String, u32>,
    /// 每阶段的实际投入量，用于分次投料与防止重复投入。
    #[serde(default)]
    pub by_stage: HashMap<usize, HashMap<String, u32>>,
    /// 已完成（on-time 投入）的阶段 index（按 recipe.stages 顺序）。
    pub completed_stages: Vec<usize>,
    /// 错过的阶段 index。
    pub missed_stages: Vec<usize>,
    /// plan-shelflife-v1 §5.1 M5c — 当下 current_qi 聚合（所有投入的 weighted average）。
    /// 1.0 = 所有材料全鲜；0.5 = 整体半衰；0.0 = 全死物。
    /// 无 freshness 物品 caller 传 factor=1.0 保持原行为。
    /// 默认 1.0 用于 legacy 持久化兼容。
    #[serde(default = "default_quality_factor")]
    pub quality_factor: f32,
    /// 累积投入总数（running weighted average 分母）。
    #[serde(default)]
    pub quality_total_count: u32,
}

fn default_quality_factor() -> f32 {
    1.0
}

impl Default for StagedMaterials {
    fn default() -> Self {
        Self {
            materials: HashMap::new(),
            by_stage: HashMap::new(),
            completed_stages: Vec::new(),
            missed_stages: Vec::new(),
            quality_factor: 1.0,
            quality_total_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlchemySession {
    pub recipe: RecipeId,
    pub caster_id: String,
    /// 当前炉温（0..1，玩家滑块目标）。
    pub temp_current: f64,
    pub elapsed_ticks: u32,
    /// 累积 (tick, temp) 采样（每 tick 一条，用于偏差积分）。
    pub temp_track: Vec<(u32, f64)>,
    pub qi_injected: f64,
    /// 已经从玩家 `Cultivation.qi_current` 真实付款、仍由炉体账户托管的数量。
    ///
    /// `qi_injected` 还包括 AutoProfile 的炉体独立注灵，因此不能拿它直接作为
    /// 退款金额。旧存档没有这个字段时按 0 兼容；新请求只有 ledger 转账成功后才会
    /// 写入该字段。
    #[serde(default)]
    pub qi_reserved: f64,
    /// 已投入的材料（含首段起炉料 + 中途料）。
    pub staged: StagedMaterials,
    pub interventions: Vec<Intervention>,
    /// 当前香座上的香料及其剩余燃烧时间。
    #[serde(default)]
    pub incense: Option<ActiveIncense>,
    #[serde(default)]
    pub incense_burns: Vec<IncenseBurn>,
    /// 是否已结束 — 避免重复结算。
    pub finished: bool,
}

impl AlchemySession {
    pub fn new(recipe_id: RecipeId, caster_id: String) -> Self {
        Self {
            recipe: recipe_id,
            caster_id,
            temp_current: 0.0,
            elapsed_ticks: 0,
            temp_track: Vec::new(),
            qi_injected: 0.0,
            qi_reserved: 0.0,
            staged: StagedMaterials::default(),
            interventions: Vec::new(),
            incense: None,
            incense_burns: Vec::new(),
            finished: false,
        }
    }

    pub fn apply_intervention(&mut self, intervention: Intervention) {
        if self.finished {
            return;
        }
        if matches!(intervention, Intervention::AdjustTemp(value) | Intervention::InjectQi(value) if !value.is_finite())
        {
            return;
        }
        match &intervention {
            Intervention::AdjustTemp(t) => self.temp_current = t.clamp(0.0, 1.0),
            Intervention::InjectQi(q) => self.qi_injected += q.max(0.0),
            Intervention::AutoProfile(_) => { /* 预留 */ }
        }
        self.interventions.push(intervention);
        const MAX_RECENT_INTERVENTIONS: usize = 32;
        if self.interventions.len() > MAX_RECENT_INTERVENTIONS {
            self.interventions
                .drain(..self.interventions.len() - MAX_RECENT_INTERVENTIONS);
        }
    }

    /// 投入一枚香料。服务端先完成物品校验，再调用此方法写入 session。
    pub fn place_incense(&mut self, kind: String, effect: IncenseEffect) -> Result<(), String> {
        if self.finished {
            return Err("本炉已经结束".into());
        }
        if self.incense_burns.len() >= MAX_INCENSE_BURNS {
            return Err("香料燃烧记录已达到上限".into());
        }
        if self.incense_active().is_some() {
            return Err("香座上仍有未燃尽的香".into());
        }
        self.incense_burns.push(IncenseBurn {
            start_tick: self.elapsed_ticks,
            burned_ticks: 0,
            effect: effect.clone(),
        });
        self.incense = Some(ActiveIncense {
            kind,
            remaining_ticks: effect.duration_ticks,
            effect,
        });
        Ok(())
    }

    pub fn incense_active(&self) -> Option<&ActiveIncense> {
        self.incense
            .as_ref()
            .filter(|incense| incense.remaining_ticks > 0)
    }

    pub fn incense_temp_band_scale(&self) -> f64 {
        self.incense_active()
            .map(|incense| incense.effect.temp_band_scale)
            .unwrap_or(1.0)
    }

    pub fn incense_qi_cost_scale(&self) -> f64 {
        self.average_incense_scale(|effect| effect.qi_cost_scale)
    }

    pub fn incense_qi_gain_scale(&self) -> f64 {
        self.average_incense_scale(|effect| effect.qi_gain_scale)
    }

    fn average_incense_scale(&self, scale: impl Fn(&IncenseEffect) -> f64) -> f64 {
        if self.elapsed_ticks == 0 {
            return 1.0;
        }
        let adjustment: f64 = self
            .incense_burns
            .iter()
            .map(|burn| f64::from(burn.burned_ticks) * (scale(&burn.effect) - 1.0))
            .sum();
        1.0 + adjustment / f64::from(self.elapsed_ticks)
    }

    fn temp_band_scale_at(&self, tick: u32) -> f64 {
        self.incense_burns
            .iter()
            .find(|burn| tick >= burn.start_tick && tick - burn.start_tick < burn.burned_ticks)
            .map(|burn| burn.effect.temp_band_scale)
            .unwrap_or(1.0)
    }

    /// 记录一笔已经由守恒账本结算成功的玩家注灵。
    ///
    /// 该方法故意与 `apply_intervention(InjectQi)` 分开：AutoProfile 使用炉体自己的
    /// `FurnaceQiReserve`，没有玩家付款，不能进入可退款的 `qi_reserved`。
    pub fn record_paid_qi(&mut self, amount: f64) {
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        self.qi_injected += amount;
        self.qi_reserved += amount;
        self.interventions.push(Intervention::InjectQi(amount));
    }

    /// 在对应 ledger 转账成功后，移除 session 的已付款统计。
    ///
    /// 调用方必须先成功完成 `transfer_ledger_qi_to_external` 或等价的炉体转出，
    /// 再调用此方法；这样 `qi_reserved` 始终与炉体账户余额同步，不会先扣字段再
    /// 因 ledger 失败而丢失可退款记录。
    pub fn remove_paid_qi_after_transfer(&mut self, amount: f64) {
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let removed = amount.min(self.qi_reserved.max(0.0));
        self.qi_reserved -= removed;
        self.qi_injected = (self.qi_injected - removed).max(0.0);
    }

    /// plan §1.3 投料记录 — 起炉投料（stage0）或中途投料。
    /// 返回 Err 若投料时机超窗（missed）。
    ///
    /// M5c：`materials` 每项 `(name, count, quality_factor)` — `quality_factor` 是
    /// `shelflife::decay_current_qi_factor` 结果（无 Freshness / 凡俗物品 caller 传 1.0）。
    /// 投入累加到 `staged.quality_factor` 的 running weighted average（by count）。
    pub fn feed_stage(
        &mut self,
        recipe: &Recipe,
        stage_idx: usize,
        materials: &[(String, u32, f32)],
    ) -> Result<(), String> {
        let stage = recipe
            .stages
            .get(stage_idx)
            .ok_or_else(|| format!("stage {stage_idx} out of range"))?;
        let tick = self.elapsed_ticks;
        // window=0 + at_tick>0 的 stage 视为硬窗口，必须精确 tick
        let start = stage.at_tick;
        let end = stage.at_tick.saturating_add(stage.window);
        if tick < start || tick > end {
            self.staged.missed_stages.push(stage_idx);
            return Err(format!(
                "stage {stage_idx} window missed: tick={tick} window=[{start},{end}]"
            ));
        }
        // Codex P2 (PR #39) — legacy session 兼容：反序列化旧档时 quality_total_count
        // 默认 0、quality_factor 默认 1.0，但 `materials` 可能已有投料。首次投新料前
        // 先按 `materials.sum()` 补齐 total_count（假设旧档投料全鲜），否则新投料会
        // 把 prev_total 错算成 0，running avg 被后续 factor 过度拉低。
        self.reconcile_legacy_quality_count();
        if self.staged.by_stage.is_empty() && !self.staged.materials.is_empty() {
            let mut by_stage = HashMap::<usize, HashMap<String, u32>>::new();
            for (index, stage) in recipe.stages.iter().enumerate() {
                for ingredient in &stage.required {
                    let count = self.stage_material_count(recipe, index, &ingredient.material);
                    if count > 0 {
                        by_stage
                            .entry(index)
                            .or_default()
                            .insert(ingredient.material.clone(), count);
                    }
                }
            }
            self.staged.by_stage = by_stage;
        }
        for (mat, count, factor) in materials {
            *self.staged.materials.entry(mat.clone()).or_insert(0) += *count;
            *self
                .staged
                .by_stage
                .entry(stage_idx)
                .or_default()
                .entry(mat.clone())
                .or_default() += *count;
            // M5c — update running weighted average：acc_new = (acc_prev × n_prev + factor × count) / n_new
            let prev_total = self.staged.quality_total_count;
            let new_total = prev_total.saturating_add(*count);
            if new_total > 0 {
                let prev_sum = (self.staged.quality_factor as f64) * (prev_total as f64);
                let add_sum = (*factor as f64) * (*count as f64);
                self.staged.quality_factor = ((prev_sum + add_sum) / (new_total as f64)) as f32;
                self.staged.quality_total_count = new_total;
            }
        }
        let complete = stage.required.iter().all(|ingredient| {
            self.stage_material_count(recipe, stage_idx, &ingredient.material) >= ingredient.count
        });
        if complete && !self.staged.completed_stages.contains(&stage_idx) {
            self.staged.completed_stages.push(stage_idx);
        }
        Ok(())
    }

    pub fn stage_material_count(&self, recipe: &Recipe, stage: usize, material: &str) -> u32 {
        if let Some(items) = self.staged.by_stage.get(&stage) {
            return items.get(material).copied().unwrap_or(0);
        }
        // 旧档只保存累计材料，按已投阶段的先后分配；不能把旧材料当作尚未投入。
        let mut remaining = self.staged.materials.get(material).copied().unwrap_or(0);
        for (index, hint) in recipe.stages.iter().enumerate().take(stage + 1) {
            if !self.staged.completed_stages.contains(&index) {
                continue;
            }
            let required = hint
                .required
                .iter()
                .filter(|item| item.material == material)
                .map(|item| item.count)
                .sum::<u32>();
            let inserted = self
                .staged
                .by_stage
                .get(&index)
                .and_then(|items| items.get(material))
                .copied()
                .unwrap_or(required.min(remaining));
            if index == stage {
                return inserted;
            }
            remaining = remaining.saturating_sub(inserted);
        }
        0
    }

    /// Codex P2 (PR #39) — 反序列化旧档兼容：若 `quality_total_count==0` 但 `materials`
    /// 非空，按 materials.sum() 补齐，avoiding running avg 对历史投料的遗漏。
    /// 幂等：`quality_total_count > 0` 时 no-op，不重复补。
    fn reconcile_legacy_quality_count(&mut self) {
        if self.staged.quality_total_count == 0 && !self.staged.materials.is_empty() {
            self.staged.quality_total_count = self.staged.materials.values().sum();
        }
    }

    /// 首料齐备后开始计时；后续阶段仍按各自的时间窗口投入。
    pub fn ready_to_heat(&self, recipe: &Recipe) -> bool {
        if self.elapsed_ticks > 0 {
            return true;
        }
        let mut required = HashMap::<&str, u32>::new();
        for ingredient in recipe
            .stages
            .iter()
            .filter(|stage| stage.at_tick == 0)
            .flat_map(|stage| &stage.required)
        {
            *required.entry(&ingredient.material).or_default() += ingredient.count;
        }
        required.into_iter().all(|(material, count)| {
            self.staged.materials.get(material).copied().unwrap_or(0) >= count
        })
    }

    /// 每 tick 调用一次 — 推进时间、采样温度。
    pub fn tick(&mut self) {
        if self.finished {
            return;
        }
        self.temp_track
            .push((self.elapsed_ticks, self.temp_current));
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if let Some(incense) = self.incense.as_mut() {
            if incense.remaining_ticks > 0 {
                if let Some(burn) = self.incense_burns.last_mut() {
                    burn.burned_ticks = burn.burned_ticks.saturating_add(1);
                }
            }
        }
        self.tick_incense();
    }

    /// 香继续燃烧，首料等待和到时停炉不暂停计时。
    pub fn tick_incense(&mut self) {
        if let Some(incense) = self.incense.as_mut() {
            incense.remaining_ticks = incense.remaining_ticks.saturating_sub(1);
        }
    }

    /// plan §1.3 结算 — 计算 DeviationSummary + OutcomeBucket。
    /// `precise=true` 意味着投入材料精确匹配该 recipe；false 走残缺匹配路径（由外层决定）。
    pub fn summarize(&self, recipe: &Recipe) -> DeviationSummary {
        self.summarize_with_alchemy_effective_lv(recipe, 0)
    }

    pub fn summarize_with_alchemy_effective_lv(
        &self,
        recipe: &Recipe,
        alchemy_effective_lv: u8,
    ) -> DeviationSummary {
        let mut scaled_profile = recipe.fire_profile.clone();
        let tol_scale = f64::from(tolerance_scale(alchemy_effective_lv));
        scaled_profile.tolerance.temp_band *= tol_scale;
        scaled_profile.tolerance.duration_band = ((scaled_profile.tolerance.duration_band as f64)
            * tol_scale)
            .round()
            .max(1.0) as u32;

        let mut deviation_sum = 0.0;
        let mut severe_overheat = false;
        for (tick, temp) in &self.temp_track {
            let band =
                (scaled_profile.tolerance.temp_band * self.temp_band_scale_at(*tick)).max(1e-6);
            let over = (temp - recipe.fire_profile.target_temp).abs();
            deviation_sum += (over / band - 1.0).max(0.0);
            severe_overheat |= temp - recipe.fire_profile.target_temp > band * 3.0;
        }
        let temp_deviation = if self.temp_track.is_empty() {
            f64::INFINITY
        } else {
            deviation_sum / self.temp_track.len() as f64
        };
        let duration_deviation = compute_duration_deviation(self.elapsed_ticks, &scaled_profile);
        // 未完成的必要阶段
        let missed_stage = !self.staged.missed_stages.is_empty()
            || recipe
                .stages
                .iter()
                .enumerate()
                .any(|(i, _)| !self.staged.completed_stages.contains(&i));
        // qi 不足：注入量 < recipe.fire_profile.qi_cost
        let qi_deficit =
            self.qi_injected + 1e-9 < recipe.fire_profile.qi_cost * self.incense_qi_cost_scale();
        DeviationSummary {
            temp_deviation,
            duration_deviation,
            missed_stage,
            qi_deficit,
            severe_overheat,
        }
    }

    pub fn classify(&self, recipe: &Recipe) -> OutcomeBucket {
        self.classify_with_alchemy_effective_lv(recipe, 0)
    }

    pub fn classify_with_alchemy_effective_lv(
        &self,
        recipe: &Recipe,
        alchemy_effective_lv: u8,
    ) -> OutcomeBucket {
        classify_precise(&self.summarize_with_alchemy_effective_lv(recipe, alchemy_effective_lv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, PillOutcome, Recipe, RecipeStage, ToleranceSpec,
    };
    use crate::cultivation::components::ColorKind;

    fn simple_single_stage_recipe() -> Recipe {
        Recipe {
            id: "r".into(),
            name: "r".into(),
            furnace_tier_min: 1,
            stages: vec![RecipeStage {
                at_tick: 0,
                required: vec![IngredientSpec {
                    material: "m".into(),
                    count: 1,
                    mineral_id: None,
                }],
                window: 0,
            }],
            fire_profile: FireProfile {
                target_temp: 0.5,
                target_duration_ticks: 10,
                qi_cost: 5.0,
                tolerance: ToleranceSpec {
                    temp_band: 0.1,
                    duration_band: 2,
                },
            },
            outcomes: Outcomes {
                perfect: Some(PillOutcome {
                    pill: "pill".into(),
                    quality: 1.0,
                    toxin_amount: 0.2,
                    toxin_color: ColorKind::Mellow,
                    qi_gain: None,
                }),
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        }
    }

    #[test]
    fn legacy_materials_survive_partial_feed_across_ingredients_and_stages() {
        let mut recipe = simple_single_stage_recipe();
        recipe.stages[0].required[0].count = 2;
        recipe.stages[0].required.push(IngredientSpec {
            material: "n".into(),
            count: 1,
            mineral_id: None,
        });
        recipe.stages.push(RecipeStage {
            at_tick: 5,
            required: vec![IngredientSpec {
                material: "m".into(),
                count: 3,
                mineral_id: None,
            }],
            window: 5,
        });
        let mut session = AlchemySession::new(recipe.id.clone(), "alice".into());
        session.staged.materials = HashMap::from([("m".into(), 3), ("n".into(), 1)]);
        session.staged.completed_stages = vec![0, 1];
        session.elapsed_ticks = 5;

        // 旧档没有逐阶段字段；累计的三份 m 应分成首段两份、次段一份。
        let mut saved = serde_json::to_value(&session).unwrap();
        saved["staged"].as_object_mut().unwrap().remove("by_stage");
        let mut restored: AlchemySession = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.stage_material_count(&recipe, 0, "m"), 2);
        assert_eq!(restored.stage_material_count(&recipe, 1, "m"), 1);
        restored
            .feed_stage(&recipe, 1, &[("m".into(), 1, 1.0)])
            .unwrap();
        assert_eq!(
            restored.stage_material_count(&recipe, 0, "n"),
            1,
            "迁移不能漏掉同一阶段的第二种药材"
        );
        assert_eq!(restored.stage_material_count(&recipe, 0, "m"), 2);
        assert_eq!(restored.stage_material_count(&recipe, 1, "m"), 2);
        restored
            .feed_stage(&recipe, 1, &[("m".into(), 1, 1.0)])
            .unwrap();
        assert_eq!(
            restored.stage_material_count(&recipe, 1, "m"),
            3,
            "分次投料应只累加到目标阶段"
        );
    }

    #[test]
    fn plain_incense_counts_down_while_waiting_without_changing_the_brew() {
        let recipe = simple_single_stage_recipe();
        let mut session = AlchemySession::new(recipe.id.clone(), "alice".into());
        let effect = super::super::incense::effect_for_item("incense_plain").unwrap();
        let duration = effect.duration_ticks;
        session
            .place_incense("incense_plain".into(), effect)
            .unwrap();
        session.tick_incense();
        assert_eq!(session.elapsed_ticks, 0);
        assert_eq!(
            session.incense_active().unwrap().remaining_ticks,
            duration - 1
        );
        session.tick();
        assert_eq!(session.incense_temp_band_scale(), 1.0);
        assert_eq!(session.incense_qi_cost_scale(), 1.0);
        assert_eq!(session.incense_qi_gain_scale(), 1.0);
        session.finished = true;
        for _ in 0..duration {
            session.tick_incense();
        }
        assert!(
            session.incense_active().is_none(),
            "停炉等待收取期间香仍应燃尽"
        );
        assert_eq!(session.elapsed_ticks, 1, "香料计时不能延长炼制时长");
    }

    #[test]
    fn incense_burn_history_has_a_bounded_capacity() {
        let recipe = simple_single_stage_recipe();
        let mut session = AlchemySession::new(recipe.id.clone(), "alice".into());
        let effect = super::super::incense::effect_for_item("incense_plain").unwrap();
        for _ in 0..super::MAX_INCENSE_BURNS {
            session
                .place_incense("incense_plain".into(), effect.clone())
                .unwrap();
            session.incense = None;
        }
        let rejected = session.place_incense("incense_plain".into(), effect);
        assert!(rejected.is_err(), "超过上限的香料记录必须被拒绝");
        assert_eq!(session.incense_burns.len(), super::MAX_INCENSE_BURNS);
    }

    #[test]
    fn incense_changes_authoritative_tolerance_and_qi_cost() {
        let recipe = simple_single_stage_recipe();
        let mut clear = AlchemySession::new(recipe.id.clone(), "alice".into());
        clear
            .place_incense(
                "incense_clear_mind".into(),
                super::super::incense::effect_for_item("incense_clear_mind").unwrap(),
            )
            .unwrap();
        assert!((clear.incense_temp_band_scale() - 1.25).abs() < 1e-9);
        assert!((clear.incense_qi_cost_scale() - 1.0).abs() < 1e-9);

        let mut warm = AlchemySession::new(recipe.id.clone(), "alice".into());
        warm.place_incense(
            "incense_warm_ash".into(),
            super::super::incense::effect_for_item("incense_warm_ash").unwrap(),
        )
        .unwrap();
        assert_eq!(
            warm.incense_qi_cost_scale(),
            1.0,
            "尚未燃烧不能改变整炉真元需求"
        );
        assert!(warm
            .place_incense(
                "incense_damp_wood".into(),
                super::super::incense::effect_for_item("incense_damp_wood").unwrap(),
            )
            .is_err());
        for _ in 0..200 {
            warm.tick();
        }
        assert!(
            (warm.incense_qi_cost_scale() - 0.85).abs() < 1e-9,
            "燃尽后必须保留已产生的香效"
        );
        assert!(warm
            .place_incense(
                "incense_damp_wood".into(),
                super::super::incense::effect_for_item("incense_damp_wood").unwrap(),
            )
            .is_ok());
    }

    fn multi_stage_recipe() -> Recipe {
        let mut r = simple_single_stage_recipe();
        r.stages.push(RecipeStage {
            at_tick: 5,
            required: vec![IngredientSpec {
                material: "n".into(),
                count: 1,
                mineral_id: None,
            }],
            window: 2,
        });
        r
    }

    #[test]
    fn incense_affects_only_ticks_when_it_was_burning() {
        let recipe = simple_single_stage_recipe();
        let mut early = AlchemySession::new(recipe.id.clone(), "alice".into());
        early.temp_current = recipe.fire_profile.target_temp + 0.2;
        let mut late = early.clone();
        let mut effect = super::super::incense::effect_for_item("incense_clear_mind").unwrap();
        effect.duration_ticks = 5;
        effect.qi_cost_scale = 0.8;
        early
            .place_incense("incense_clear_mind".into(), effect.clone())
            .unwrap();
        for _ in 0..10 {
            early.tick();
            late.tick();
        }
        late.place_incense("incense_clear_mind".into(), effect)
            .unwrap();
        assert!(
            early.summarize(&recipe).temp_deviation < late.summarize(&recipe).temp_deviation,
            "燃尽的香效仍影响对应温度采样；最后才投香不能回改过去"
        );
        assert!(
            (early.incense_qi_cost_scale() - 0.9).abs() < 1e-9,
            "半程燃烧应只获得半程真元需求调整"
        );
        assert_eq!(late.incense_qi_cost_scale(), 1.0);
    }

    #[test]
    fn feed_stage0_on_start_succeeds() {
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        assert_eq!(s.staged.materials["m"], 1);
        assert_eq!(s.staged.completed_stages, vec![0]);
    }

    #[test]
    fn feed_stage_outside_window_records_miss() {
        let r = multi_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        // 不做 stage0 → tick forward past stage1 window
        for _ in 0..20 {
            s.tick();
        }
        let err = s.feed_stage(&r, 1, &[("n".into(), 1, 1.0)]);
        assert!(err.is_err());
        assert_eq!(s.staged.missed_stages, vec![1]);
    }

    #[test]
    fn tick_accumulates_temp_track_and_elapsed() {
        let mut s = AlchemySession::new("r".into(), "alice".into());
        s.apply_intervention(Intervention::AdjustTemp(0.5));
        s.tick();
        s.tick();
        assert_eq!(s.elapsed_ticks, 2);
        assert_eq!(s.temp_track.len(), 2);
        assert!((s.temp_track[0].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn perfect_run_end_to_end() {
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.5));
        s.apply_intervention(Intervention::InjectQi(5.0));
        for _ in 0..10 {
            s.tick();
        }
        let bucket = s.classify(&r);
        assert_eq!(bucket, OutcomeBucket::Perfect);
    }

    #[test]
    fn missed_stage_produces_flawed_or_worse() {
        let r = multi_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.5));
        s.apply_intervention(Intervention::InjectQi(5.0));
        for _ in 0..10 {
            s.tick();
        }
        // 未投 stage1 → missed
        let bucket = s.classify(&r);
        assert!(matches!(
            bucket,
            OutcomeBucket::Flawed | OutcomeBucket::Waste
        ));
    }

    #[test]
    fn qi_deficit_routes_to_waste() {
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.5));
        // no qi injected
        for _ in 0..10 {
            s.tick();
        }
        assert_eq!(s.classify(&r), OutcomeBucket::Waste);
    }

    #[test]
    fn severe_overheat_explodes() {
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        s.apply_intervention(Intervention::InjectQi(5.0));
        s.apply_intervention(Intervention::AdjustTemp(1.0)); // 远超 target 0.5, band 0.1
        for _ in 0..10 {
            s.tick();
        }
        assert_eq!(s.classify(&r), OutcomeBucket::Explode);
    }

    #[test]
    fn cold_furnace_can_fail_but_does_not_explode_from_overheating() {
        let recipe = simple_single_stage_recipe();
        let mut session = AlchemySession::new(recipe.id.clone(), "alice".into());
        session.tick();
        assert!(!session.summarize(&recipe).severe_overheat);
        assert_ne!(session.classify(&recipe), OutcomeBucket::Explode);
    }

    #[test]
    fn higher_alchemy_skill_expands_tolerance_and_improves_bucket() {
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.75));
        s.apply_intervention(Intervention::InjectQi(5.0));
        for _ in 0..10 {
            s.tick();
        }

        assert_eq!(
            s.classify_with_alchemy_effective_lv(&r, 0),
            OutcomeBucket::Good
        );
        assert_eq!(
            s.classify_with_alchemy_effective_lv(&r, 10),
            OutcomeBucket::Perfect
        );
    }

    #[test]
    fn fresh_plan_matches_du_ming_san_stage_structure() {
        // 兜底：确认 multi-stage 测试和 du_ming_san 的预期 3 阶段一致
        let r = multi_stage_recipe();
        assert_eq!(r.stages.len(), 2);
        assert_eq!(r.stages[1].window, 2);
    }

    // ============== Codex P2 (PR #39) — legacy session recovery ==============

    #[test]
    fn legacy_session_recovery_reconciles_quality_total_count() {
        // 模拟 pre-M5c 档序列化恢复：materials 已有 3 份投料，但 quality_total_count
        // 默认为 0（serde default），quality_factor 默认 1.0。现在继续投 1 份 factor=0.5
        // 新料：正确结果应是 (3×1.0 + 1×0.5)/4 = 0.875，而非 bug 版的 0.5。
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        // 模拟反序列化后 state
        s.staged.materials.insert("a".into(), 3);
        assert_eq!(s.staged.quality_total_count, 0);
        assert_eq!(s.staged.quality_factor, 1.0);

        // 首次投料应先 reconcile legacy count，再 update running avg
        s.feed_stage(&r, 0, &[("b".into(), 1, 0.5)]).unwrap();

        assert_eq!(s.staged.quality_total_count, 4);
        assert!(
            (s.staged.quality_factor - 0.875).abs() < 1e-3,
            "legacy recovery got {}",
            s.staged.quality_factor
        );
    }

    #[test]
    fn reconcile_quality_count_idempotent_when_already_set() {
        // 新 session 正常投料后，再投一次不应重复 reconcile（count 已非 0）。
        let r = simple_single_stage_recipe();
        let mut s = AlchemySession::new(r.id.clone(), "alice".into());
        s.feed_stage(&r, 0, &[("m".into(), 1, 1.0)]).unwrap();
        assert_eq!(s.staged.quality_total_count, 1);
        // 第二次投料（materials 已有 1 但 count 也是 1 — reconcile 无效果）
        s.feed_stage(&r, 0, &[("m".into(), 2, 0.5)]).unwrap();
        // 期望：(1×1.0 + 2×0.5)/3 = 0.667；若 reconcile 错误叠加会得不同值
        assert_eq!(s.staged.quality_total_count, 3);
        assert!(
            (s.staged.quality_factor - 0.6667).abs() < 1e-3,
            "got {}",
            s.staged.quality_factor
        );
    }
}
