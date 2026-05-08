//! 角色生命周期判定（plan-multi-life-v1 P0 主体 + §3 数据契约第 2 条）。
//!
//! 当玩家死亡时，本模块的 [`regenerate_or_terminate`] 决定下一步是 **重生**
//! 还是 **角色终结进入 character_select**。判定逻辑严格遵循 plan-multi-life-v1
//! §2 流程：
//!
//! 1. 自然老死（[`CultivationDeathCause::NaturalAging`]）→ `Terminate(NaturalAging)`。
//!    底层 `combat::lifecycle::apply_natural_aging_lifespan_exhaustion` 会同时把
//!    `years_lived` 推到 cap，使 lifespan 也满足 exhausted；本决策器把 cause 优先
//!    判，保留"老死"语义而不被 LifespanExhausted 抢占。
//! 2. 寿元 ≤ 0（被杀 / 渡劫失败 + 死亡扣寿后 cap 归零）→ `Terminate(LifespanExhausted)`
//! 3. 运数池耗尽（fortune == 0）→ `Terminate(FortuneExhausted)`
//! 4. 否则 → `Revive(remaining_fortune)`，调用方应用 [`crate::cultivation::luck_pool::spend_fortune`]
//!    扣 1 后让玩家在灵龛重生
//!
//! ## 当前接入状态（plan-multi-life-v1 P0）
//!
//! **本函数是接口先于实装**。生产路径的死亡 → 重生/终结决策仍由
//! `combat::lifecycle::determine_revival_decision` 做出（含 `RebirthChanceInput`
//! / 灵龛归属 / 渡劫概率 / karma 等更复杂的输入），而非本函数。本模块作为
//! plan-multi-life-v1 §3 数据契约第 2 条要求的 **决策语义层** 落地，提供：
//!
//! 1. 单一职责的纯决策入口（无副作用，便于单测覆盖每条状态转换）；
//! 2. 与 [`crate::cultivation::luck_pool`] 配套的 multi-life 语义门面；
//! 3. plan §2 流程在代码层的可执行验证（17 单测 / 4 集成测试链路）。
//!
//! 后续 P+1 阶段如需让生产路径切到新决策器，可在 `combat::lifecycle::handle_cultivation_death_triggers`
//! 中替换 `determine_revival_decision` 的"是否完全不重生"分支为本函数——届时
//! 本模块的所有测试与现有 17 case 应自动锁住语义不漂移（CLAUDE.md "Testing —
//! 饱和化测试"第 3 条："mock 顶位时接口必须完整 ... 真实 impl 接入时只换 impl
//! 不改测试"）。

use crate::combat::components::Lifecycle;
use crate::cultivation::death_hooks::CultivationDeathCause;
use crate::cultivation::lifespan::LifespanComponent;
use crate::cultivation::luck_pool;

/// 终结原因（plan-multi-life-v1 §2 流程节点）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminateReason {
    /// 自然老死（years_lived ≥ cap，等价 `NaturalAging` cause）。
    NaturalAging,
    /// 死亡扣寿后寿元归零（plan-lifespan-v1 §2 死亡扣寿公式触发）。
    LifespanExhausted,
    /// 运数池耗尽（per-life fortune 用完，plan §0 O.4 决议）。
    FortuneExhausted,
}

/// 角色生命周期 tick 后的结果（plan-multi-life-v1 §2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeOutcome {
    /// 重生：调用方应当扣 1 点运数（[`luck_pool::spend_fortune`]）并把玩家
    /// 状态切回 `LifecycleState::AwaitingRevival`。
    /// `remaining_fortune_after_spend` 是扣完之后剩的值（用于 UI 显示）。
    Revive { remaining_fortune_after_spend: u8 },
    /// 角色终结：调用方应当走 `terminate_lifecycle` + `emit_terminate_screen`，
    /// 等待用户点 "再来一世" 触发 `RevivalActionKind::CreateNewCharacter`。
    Terminate { reason: TerminateReason },
}

impl LifeOutcome {
    pub fn is_terminate(self) -> bool {
        matches!(self, Self::Terminate { .. })
    }

    pub fn is_revive(self) -> bool {
        matches!(self, Self::Revive { .. })
    }
}

/// 决策：当前死亡是否触发"角色终结进入 character_select"。
///
/// 输入：当前死亡原因 + 寿元 + 运数池状态。本函数 **纯决策**，不修改入参。
///
/// 调用方典型流程（伪码）：
/// ```ignore
/// match regenerate_or_terminate(&lifespan, &lifecycle, cause) {
///     LifeOutcome::Revive { .. } => {
///         luck_pool::spend_fortune(&mut lifecycle); // 实际扣减
///         lifecycle.enter_near_death(now_tick);
///     }
///     LifeOutcome::Terminate { reason } => {
///         terminate_lifecycle(...);
///     }
/// }
/// ```
pub fn regenerate_or_terminate(
    lifespan: &LifespanComponent,
    lifecycle: &Lifecycle,
    cause: CultivationDeathCause,
) -> LifeOutcome {
    // plan §2 第 1 条：自然老死即终结。NaturalAging 在调用前已被
    // `apply_natural_aging_lifespan_exhaustion` 推满 years_lived，但此处仍按
    // cause 优先判定，保留"老死"语义不被后续的 LifespanExhausted 抢占。
    if cause == CultivationDeathCause::NaturalAging {
        return LifeOutcome::Terminate {
            reason: TerminateReason::NaturalAging,
        };
    }

    // plan §2 + plan-lifespan-v1 §2：寿元归零强制终结
    // remaining_years() 返回 (cap - years_lived).max(0.0)，浮点比较用 epsilon
    if lifespan.remaining_years() <= f64::EPSILON {
        return LifeOutcome::Terminate {
            reason: TerminateReason::LifespanExhausted,
        };
    }

    // plan §0 第 1 条 + §2："运数耗尽 → 角色终结"
    if luck_pool::is_exhausted(lifecycle) {
        return LifeOutcome::Terminate {
            reason: TerminateReason::FortuneExhausted,
        };
    }

    // 否则重生：返回扣减后预期剩余值（调用方实际执行 spend_fortune）
    let remaining_after_spend = luck_pool::current_fortune(lifecycle).saturating_sub(1);
    LifeOutcome::Revive {
        remaining_fortune_after_spend: remaining_after_spend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::Lifecycle;
    use crate::cultivation::lifespan::{LifespanCapTable, LifespanComponent};
    use crate::cultivation::luck_pool::INITIAL_FORTUNE_PER_LIFE;

    fn fresh_lifecycle() -> Lifecycle {
        Lifecycle {
            character_id: "offline:Tester:gen0".to_string(),
            ..Lifecycle::default()
        }
    }

    fn fresh_lifespan() -> LifespanComponent {
        LifespanComponent::new(LifespanCapTable::AWAKEN)
    }

    // ---------------- happy path ----------------

    #[test]
    fn alive_with_fortune_revives_and_returns_remaining_minus_one() {
        let lifespan = fresh_lifespan();
        let lifecycle = fresh_lifecycle();

        let outcome = regenerate_or_terminate(
            &lifespan,
            &lifecycle,
            CultivationDeathCause::NegativeZoneDrain,
        );

        // 起始 fortune=3，扣减后预期剩 2
        assert_eq!(
            outcome,
            LifeOutcome::Revive {
                remaining_fortune_after_spend: 2,
            },
        );
        assert!(outcome.is_revive());
    }

    // ---------------- 状态转换：fortune 边界 ----------------

    #[test]
    fn fortune_one_revives_to_zero() {
        let lifespan = fresh_lifespan();
        let mut lc = fresh_lifecycle();
        lc.fortune_remaining = 1;

        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);

        assert_eq!(
            outcome,
            LifeOutcome::Revive {
                remaining_fortune_after_spend: 0,
            },
            "fortune=1 仍触发重生（运数还在），扣减后 = 0",
        );
    }

    #[test]
    fn fortune_zero_terminates_with_fortune_exhausted() {
        let lifespan = fresh_lifespan();
        let mut lc = fresh_lifecycle();
        lc.fortune_remaining = 0;

        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::FortuneExhausted,
            },
            "fortune=0 + 非自然死 + 寿元未归零 = FortuneExhausted",
        );
        assert!(outcome.is_terminate());
    }

    // ---------------- 状态转换：lifespan 边界 ----------------

    #[test]
    fn lifespan_exhausted_overrides_fortune_remaining() {
        // 寿元归零优先于运数判定（plan §2 流程顺序：寿元归零 OR 运数耗尽）
        let mut lifespan = fresh_lifespan();
        lifespan.years_lived = lifespan.cap_by_realm as f64;
        let lc = fresh_lifecycle(); // fortune=3 完整

        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::LifespanExhausted,
            },
            "寿元归零即终结，无视运数池剩余",
        );
    }

    #[test]
    fn lifespan_almost_exhausted_still_revives() {
        // 边界 off-by-one 测试：剩 1 年（remaining_years == 1.0）应当能重生
        let mut lifespan = fresh_lifespan();
        lifespan.years_lived = lifespan.cap_by_realm as f64 - 1.0;
        let lc = fresh_lifecycle();

        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);

        assert!(
            outcome.is_revive(),
            "剩余寿元 > epsilon 时应能重生（边界条件）",
        );
    }

    // ---------------- enum 变体专属覆盖 ----------------

    #[test]
    fn natural_aging_always_terminates_regardless_of_fortune() {
        // NaturalAging 跳过运数检查（regenerate_or_terminate 第一个分支即返回 Terminate(NaturalAging)）
        let lifespan = fresh_lifespan();
        let lc = fresh_lifecycle(); // fortune=3 满

        let outcome = regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NaturalAging);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::NaturalAging,
            },
            "NaturalAging 不消耗运数，直接终结",
        );
    }

    #[test]
    fn natural_aging_terminates_even_when_fortune_zero() {
        // 即便运数 = 0，原因仍报 NaturalAging（不应被 FortuneExhausted 抢占）
        let lifespan = fresh_lifespan();
        let mut lc = fresh_lifecycle();
        lc.fortune_remaining = 0;

        let outcome = regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NaturalAging);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::NaturalAging,
            },
            "NaturalAging 优先级高于 FortuneExhausted（决策语义『老死』应保留）",
        );
    }

    #[test]
    fn all_non_natural_causes_consume_fortune_normally() {
        // CultivationDeathCause 共 6 variant，验证除 NaturalAging 外的 5 种都走运数池
        let lifespan = fresh_lifespan();
        let lc = fresh_lifecycle();

        for cause in [
            CultivationDeathCause::BreakthroughBackfire,
            CultivationDeathCause::MeridianCollapse,
            CultivationDeathCause::NegativeZoneDrain,
            CultivationDeathCause::ContaminationOverflow,
            CultivationDeathCause::SwarmQiDrain,
        ] {
            let outcome = regenerate_or_terminate(&lifespan, &lc, cause);
            assert!(
                outcome.is_revive(),
                "非 NaturalAging 死因 {cause:?} + fortune>0 + 寿元未归零 应触发重生",
            );
        }
    }

    // ---------------- 多重终结触发的优先级 ----------------

    #[test]
    fn natural_aging_and_lifespan_exhausted_both_present_reports_natural_aging() {
        // 自然老死时通常 lifespan 也耗尽（apply_natural_aging_lifespan_exhaustion 把
        // years_lived 推满），但语义上原因是 NaturalAging。决策应优先报 NaturalAging。
        let mut lifespan = fresh_lifespan();
        lifespan.years_lived = lifespan.cap_by_realm as f64;
        let lc = fresh_lifecycle();

        let outcome = regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NaturalAging);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::NaturalAging,
            },
            "NaturalAging 优先级高于 LifespanExhausted（语义保真）",
        );
    }

    #[test]
    fn lifespan_exhausted_priority_over_fortune_exhausted() {
        // 寿元 = 0 + 运数 = 0 + 非自然死 → 报 LifespanExhausted
        let mut lifespan = fresh_lifespan();
        lifespan.years_lived = lifespan.cap_by_realm as f64;
        let mut lc = fresh_lifecycle();
        lc.fortune_remaining = 0;

        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);

        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::LifespanExhausted,
            },
            "寿元归零应当优先报告，而非 FortuneExhausted",
        );
    }

    // ---------------- LifeOutcome 辅助方法 ----------------

    #[test]
    fn life_outcome_is_terminate_helper() {
        assert!(LifeOutcome::Terminate {
            reason: TerminateReason::NaturalAging
        }
        .is_terminate());
        assert!(!LifeOutcome::Revive {
            remaining_fortune_after_spend: 1
        }
        .is_terminate());
    }

    #[test]
    fn life_outcome_is_revive_helper() {
        assert!(LifeOutcome::Revive {
            remaining_fortune_after_spend: 0
        }
        .is_revive());
        assert!(!LifeOutcome::Terminate {
            reason: TerminateReason::FortuneExhausted
        }
        .is_revive());
    }

    // ---------------- TerminateReason 全变体覆盖 ----------------

    // ---------------- P1 集成：决策 → spec 应用链路 ----------------
    //
    // plan-multi-life-v1 §1 P1 验收："寿元归零玩家自动进 character_select"。
    // 现有 combat::lifecycle 已实装：lifespan_aging_tick → CultivationDeathTrigger{NaturalAging}
    // → terminate_lifecycle_with_death_context（emit_terminate_screen）→ 用户点
    // CreateNewCharacter → reset_for_new_character（应用 character_select::next_character_spec）。
    //
    // 下面的集成测试用 character_lifecycle::regenerate_or_terminate **决策器** 与
    // character_select::next_character_spec **spec 提供器** 拼接，验证 plan §2
    // 流程语义在两者之间正确衔接：寿元归零或运数耗尽时，应同时拿到 Terminate 决策
    // 与一份合法的下一世 spec（spawn_plain / Awaken / fortune=3 / cap=AWAKEN）。

    #[test]
    fn lifespan_zero_to_new_life_spec_full_chain() {
        use crate::cultivation::character_select::next_character_spec;
        use crate::cultivation::lifespan::LifespanCapTable;

        // 玩家寿元归零（被杀 / 渡劫扣寿后到 0）
        let mut lifespan = fresh_lifespan();
        lifespan.years_lived = lifespan.cap_by_realm as f64;
        let lc = fresh_lifecycle();

        // 第 1 步：决策器报终结 (LifespanExhausted)
        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);
        assert_eq!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::LifespanExhausted,
            },
        );

        // 第 2 步：character_select 提供下一世 spec —— 必须满足 plan §2
        let spec = next_character_spec();
        assert_eq!(
            spec.realm,
            crate::cultivation::components::Realm::Awaken,
            "新角色 realm 必须 = Awaken（plan §0 第 4 条 + Q-ML4）",
        );
        assert_eq!(
            spec.lifespan_cap,
            LifespanCapTable::AWAKEN,
            "新角色寿元 cap = AWAKEN（plan §2 + Q-ML0 reframe to lifespan-v1 §2）",
        );
        assert_eq!(
            spec.initial_fortune,
            crate::cultivation::luck_pool::INITIAL_FORTUNE_PER_LIFE,
            "新角色 fortune = INITIAL_FORTUNE_PER_LIFE（plan §0 O.4）",
        );
        assert_eq!(
            spec.spawn_pos,
            crate::player::spawn_position(),
            "新角色 spawn_pos = spawn_plain（Q-ML4）",
        );
    }

    #[test]
    fn natural_aging_to_new_life_spec_full_chain() {
        // plan §2 流程：寿命归零（NaturalAging cause）→ 终结 → 提供下一世 spec
        // 即使运数池满 (fortune=3)，NaturalAging 也直接终结，不消耗运数
        use crate::cultivation::character_select::next_character_spec;

        let lifespan = fresh_lifespan(); // 寿元未归零
        let lc = fresh_lifecycle(); // fortune=3 满

        let outcome = regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NaturalAging);
        assert!(matches!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::NaturalAging
            }
        ));

        // 即便玩家寿元尚未到 cap、运数仍满，老死语义也应进 character_select
        let spec = next_character_spec();
        assert_eq!(spec.initial_fortune, INITIAL_FORTUNE_PER_LIFE);
    }

    #[test]
    fn fortune_exhausted_to_new_life_spec_resets_pool() {
        // plan §2："运数耗尽 → 角色终结 → 新角色生成 ... 运数 = 3"
        use crate::cultivation::character_select::next_character_spec;

        let lifespan = fresh_lifespan();
        let mut lc = fresh_lifecycle();
        lc.fortune_remaining = 0;

        // 决策：终结，原因 FortuneExhausted
        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);
        assert!(matches!(
            outcome,
            LifeOutcome::Terminate {
                reason: TerminateReason::FortuneExhausted
            }
        ));

        // spec 应给新角色 fortune=3（per-life pool 重置而非保留耗尽态）
        let spec = next_character_spec();
        assert_eq!(spec.initial_fortune, 3);
        assert_eq!(spec.initial_fortune, INITIAL_FORTUNE_PER_LIFE);
    }

    #[test]
    fn revive_outcome_does_not_use_new_life_spec() {
        // 反例：决策返回 Revive 时，调用方不应当读 next_character_spec
        // （spec 是"开新角色"用的，重生只 spend_fortune，不动 cultivation/realm）
        let lifespan = fresh_lifespan();
        let lc = fresh_lifecycle();
        let outcome =
            regenerate_or_terminate(&lifespan, &lc, CultivationDeathCause::NegativeZoneDrain);
        assert!(outcome.is_revive());
        // is_terminate 为 false：调用方据此跳过 spec 应用
        assert!(!outcome.is_terminate());
    }

    #[test]
    fn all_terminate_reasons_reachable() {
        // schema-pin 测试：每个 TerminateReason 变体至少一条命中 case
        let lifespan_full = fresh_lifespan();
        let mut lifespan_zero = fresh_lifespan();
        lifespan_zero.years_lived = lifespan_zero.cap_by_realm as f64;
        let lc_full = fresh_lifecycle();
        let mut lc_zero = fresh_lifecycle();
        lc_zero.fortune_remaining = 0;

        // NaturalAging
        assert!(matches!(
            regenerate_or_terminate(
                &lifespan_full,
                &lc_full,
                CultivationDeathCause::NaturalAging
            ),
            LifeOutcome::Terminate {
                reason: TerminateReason::NaturalAging
            }
        ));
        // LifespanExhausted
        assert!(matches!(
            regenerate_or_terminate(
                &lifespan_zero,
                &lc_full,
                CultivationDeathCause::NegativeZoneDrain,
            ),
            LifeOutcome::Terminate {
                reason: TerminateReason::LifespanExhausted
            }
        ));
        // FortuneExhausted
        assert!(matches!(
            regenerate_or_terminate(
                &lifespan_full,
                &lc_zero,
                CultivationDeathCause::NegativeZoneDrain,
            ),
            LifeOutcome::Terminate {
                reason: TerminateReason::FortuneExhausted
            }
        ));
    }
}
