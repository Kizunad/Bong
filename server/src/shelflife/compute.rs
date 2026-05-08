//! plan-shelflife-v1 M0 纯函数层。
//!
//! 两个核心 API：
//! - `compute_current_qi` — 按 lazy eval 算出当下 qi
//! - `compute_track_state` — 按 lazy eval 算出当下内部路径机态（非 UI 显示档位）
//!
//! # 精度注意事项
//!
//! - `effective_dt` 用 u64 承载，`(f64 * multiplier).round() as u64` 丢失亚 tick 精度。
//!   对 `half_life < 100` 的极短场景，单次 round 误差可达 5%。M0 场景（最短 half_life ≈
//!   数小时 = 数十万 tick）无实际影响。
//! - Exponential/Age 公式内部走 f32 `.powf()`，`dt as f32` 在 dt > 2^24 (~16M tick ≈ 9.7
//!   real-days @ 20 TPS) 时可能丢精度。骨币走 Linear decay ~1y，Linear 内部已转 f64 规避。
//! - Linear 公式特意走 f64 内部算 — 骨币 real-year scale decay 精度关键。

use super::types::{DecayFormula, DecayProfile, Freshness, TrackState};
use crate::qi_physics::constants::{
    QI_AMBIENT_EXCRETION_PER_SEC, QI_SHELFLIFE_DEAD_ZONE_MULTIPLIER,
};
use crate::qi_physics::{qi_excretion, ContainerKind, EnvField};
use crate::world::season::Season;

const TICKS_PER_SECOND: f64 = 20.0;

pub fn zone_multiplier_lookup(zone_qi_density: f64) -> f32 {
    if (0.0..crate::cultivation::dead_zone::DEAD_ZONE_QI_THRESHOLD).contains(&zone_qi_density) {
        QI_SHELFLIFE_DEAD_ZONE_MULTIPLIER
    } else {
        1.0
    }
}

pub fn combine_storage_and_zone_multiplier(storage_multiplier: f32, zone_qi_density: f64) -> f32 {
    storage_multiplier.max(0.0) * zone_multiplier_lookup(zone_qi_density)
}

pub fn season_decay_modifier(season: Season, entropy_seed: u64) -> f32 {
    match season {
        Season::Summer => 1.3,
        Season::Winter => 0.7,
        Season::SummerToWinter | Season::WinterToSummer => {
            let bucket = (splitmix64(entropy_seed) % 10_001) as f32 / 10_000.0;
            0.8 + bucket * 0.4
        }
    }
}

pub fn combine_storage_zone_and_season_multiplier(
    freshness: &Freshness,
    storage_multiplier: f32,
    zone_qi_density: f64,
    season: Season,
    entropy_seed: u64,
) -> f32 {
    let base = combine_storage_and_zone_multiplier(storage_multiplier, zone_qi_density);
    if base <= 0.0 || freshness.frozen_since_tick.is_some() {
        base
    } else {
        base * season_decay_modifier(season, entropy_seed)
    }
}

pub fn compute_current_qi_with_season(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> f32 {
    let multiplier = if storage_multiplier <= 0.0 || freshness.frozen_since_tick.is_some() {
        storage_multiplier
    } else {
        storage_multiplier * season_decay_modifier(season, entropy_seed)
    };
    compute_current_qi(freshness, profile, now_tick, multiplier)
}

pub fn compute_track_state_with_season(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> TrackState {
    let multiplier = if storage_multiplier <= 0.0 || freshness.frozen_since_tick.is_some() {
        storage_multiplier
    } else {
        storage_multiplier * season_decay_modifier(season, entropy_seed)
    };
    compute_track_state(freshness, profile, now_tick, multiplier)
}

/// plan §1 / §6.1 — 按 lazy eval 算物品当下灵气 / 真元 / 药力值。
///
/// # 参数
/// - `freshness` — 物品 NBT
/// - `profile` — 物品指向的 DecayProfile（调用方从 registry 按 `freshness.profile` 查出）
/// - `now_tick` — 当前 server tick
/// - `storage_multiplier` — 当前容器对衰减速率的乘子（1.0 = 无效果 / 0.5 = 玉盒 / 0.0 = 阵法护匣）
///
/// # 返回
/// - Decay / Age 路径：floor_qi 以上，衰减后不低于 floor（Age 退化为 Spoil 后按 Spoil 逻辑）
/// - Spoil 路径：可至 0
/// - Stepwise 公式：ignored dt，直接 `initial_qi * storage_multiplier`
pub fn compute_current_qi(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
) -> f32 {
    let multiplier = storage_multiplier.max(0.0);
    let effective_dt = effective_dt_ticks(freshness, now_tick, multiplier);

    match profile {
        DecayProfile::Decay {
            formula, floor_qi, ..
        } => {
            let raw = apply_formula(
                freshness.initial_qi,
                effective_dt,
                formula,
                multiplier,
                profile_container_kind(profile),
            );
            raw.max(*floor_qi)
        }
        DecayProfile::Spoil { formula, .. } => {
            let raw = apply_formula(
                freshness.initial_qi,
                effective_dt,
                formula,
                multiplier,
                profile_container_kind(profile),
            );
            raw.max(0.0)
        }
        DecayProfile::Age {
            peak_at_ticks,
            peak_bonus,
            post_peak_half_life_ticks,
            ..
        } => compute_age(
            freshness.initial_qi,
            effective_dt,
            *peak_at_ticks,
            *peak_bonus,
            *post_peak_half_life_ticks,
        )
        .max(0.0),
    }
}

pub fn profile_container_kind(profile: &DecayProfile) -> ContainerKind {
    let id = profile.id().as_str();
    if id.starts_with("bone_coin_") || id.starts_with("fauna_bone_") {
        ContainerKind::SealedInBone
    } else if id.starts_with("ling_shi_") || id == "chen_jiu_v1" || id == "chen_cu_v1" {
        ContainerKind::SealedAncientRelic
    } else if id == "ling_mu_gun_v1" {
        ContainerKind::WieldedInWeapon
    } else {
        ContainerKind::LooseInPill
    }
}

/// plan §4 / §5 — 按 lazy eval 算当下路径状态，用于 tooltip 分档 / 消费分支。
pub fn compute_track_state(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
) -> TrackState {
    let multiplier = storage_multiplier.max(0.0);
    let current = compute_current_qi(freshness, profile, now_tick, storage_multiplier);
    let initial = freshness.initial_qi.max(f32::EPSILON);

    match profile {
        DecayProfile::Decay { floor_qi, .. } => {
            if current <= *floor_qi + f32::EPSILON {
                TrackState::Dead
            } else {
                // 用 headroom-based ratio 而非原生 current/initial：确保 initial 接近 floor 的
                // 小 headroom 物品也能经过 Declining 中段而不是 Fresh → Dead 直跳。
                let headroom = initial - *floor_qi;
                let remaining = (current - *floor_qi).max(0.0);
                if headroom <= f32::EPSILON || remaining / headroom <= 0.5 {
                    TrackState::Declining
                } else {
                    TrackState::Fresh
                }
            }
        }
        DecayProfile::Spoil {
            spoil_threshold, ..
        } => {
            // 严格 `<` 语义（plan §6.3 "current_qi < spoil_threshold"）— 边界值（current == threshold）仍算 Fresh / Declining，不触发 contam 警告。
            if current < *spoil_threshold {
                TrackState::Spoiled
            } else {
                let headroom = initial - *spoil_threshold;
                let remaining = (current - *spoil_threshold).max(0.0);
                if headroom <= f32::EPSILON || remaining / headroom <= 0.5 {
                    TrackState::Declining
                } else {
                    TrackState::Fresh
                }
            }
        }
        DecayProfile::Age {
            peak_at_ticks,
            peak_window_ratio,
            post_peak_spoil_threshold,
            ..
        } => {
            // Zero-peak guard: 与 compute_current_qi / compute_age 的 peak_at_ticks == 0
            // 短路保持一致 — Current 永远是 initial，状态也应稳定 Fresh，不走 PastPeak。
            // validate() 已 reject 该情形，但防御性处理保护未经校验的 profile。
            if *peak_at_ticks == 0 {
                return TrackState::Fresh;
            }

            let effective_dt = effective_dt_ticks(freshness, now_tick, multiplier);
            let window_ratio = peak_window_ratio.clamp(0.0, 1.0);
            let peak = *peak_at_ticks as f64;
            let half_window = (peak * window_ratio as f64).round() as u64;
            let peak_lo = peak_at_ticks.saturating_sub(half_window);
            let peak_hi = peak_at_ticks.saturating_add(half_window);

            // Spoil 迁移仅在真过峰后生效（避免 malformed initial_qi < spoil_threshold 时
            // 物品一创建就误判为 AgePostPeakSpoiled）。严格 `<` 语义（plan §6.3）。
            if effective_dt > *peak_at_ticks && current < *post_peak_spoil_threshold {
                TrackState::AgePostPeakSpoiled
            } else if effective_dt >= peak_lo && effective_dt <= peak_hi {
                TrackState::Peaking
            } else if effective_dt > peak_hi {
                TrackState::PastPeak
            } else {
                TrackState::Fresh
            }
        }
    }
}

/// plan §6.1 — 扣除历史冻结 + 当前冻结区间 + 乘以容器 rate multiplier。
fn effective_dt_ticks(freshness: &Freshness, now_tick: u64, multiplier: f32) -> u64 {
    let raw_dt = now_tick.saturating_sub(freshness.created_at_tick);
    let inflight_freeze = match freshness.frozen_since_tick {
        Some(t) => now_tick.saturating_sub(t),
        None => 0,
    };
    let non_frozen_dt = raw_dt
        .saturating_sub(freshness.frozen_accumulated)
        .saturating_sub(inflight_freeze);
    ((non_frozen_dt as f64) * multiplier.max(0.0) as f64).round() as u64
}

fn apply_formula(
    initial: f32,
    effective_dt: u64,
    formula: &DecayFormula,
    multiplier: f32,
    container: ContainerKind,
) -> f32 {
    match formula {
        DecayFormula::Exponential { half_life_ticks } => {
            apply_exponential_qi_physics(initial, effective_dt, *half_life_ticks, container)
        }
        DecayFormula::Linear { decay_per_tick } => {
            // f64 内部算 — 骨币 ~1y 级 scale 时 f32 精度不够（见文件头精度注记）。
            let d = (*decay_per_tick as f64) * (effective_dt as f64);
            ((initial as f64) - d).max(0.0) as f32
        }
        DecayFormula::Stepwise => {
            // Stepwise 不用 dt；storage_multiplier 直接作用于 current。
            initial * multiplier
        }
    }
}

fn apply_exponential_qi_physics(
    initial: f32,
    effective_dt: u64,
    half_life_ticks: u64,
    container: ContainerKind,
) -> f32 {
    if half_life_ticks == 0 {
        return initial;
    }
    if initial <= 0.0 || effective_dt == 0 {
        return initial.max(0.0);
    }

    let half_life_secs = half_life_ticks as f64 / TICKS_PER_SECOND;
    let elapsed_secs = effective_dt as f64 / TICKS_PER_SECOND;
    let seal = container.seal_multiplier().max(f64::EPSILON);
    let rhythm_multiplier =
        std::f64::consts::LN_2 / (half_life_secs * QI_AMBIENT_EXCRETION_PER_SEC * seal);
    let env = EnvField {
        local_zone_qi: 0.0,
        rhythm_multiplier,
        ..EnvField::default()
    };
    qi_excretion(initial as f64, container, elapsed_secs, env) as f32
}

fn compute_age(
    initial: f32,
    effective_dt: u64,
    peak_at_ticks: u64,
    peak_bonus: f32,
    post_peak_half_life_ticks: u64,
) -> f32 {
    if peak_at_ticks == 0 {
        return initial;
    }
    let dt_f = effective_dt as f32;
    let peak_f = peak_at_ticks as f32;

    if effective_dt < peak_at_ticks {
        // 线性爬升段：initial → initial * (1 + peak_bonus)
        initial * (1.0 + peak_bonus * (dt_f / peak_f))
    } else {
        // 过峰指数衰减段
        if post_peak_half_life_ticks == 0 {
            return initial * (1.0 + peak_bonus);
        }
        let post_peak_dt = effective_dt - peak_at_ticks;
        apply_exponential_qi_physics(
            initial * (1.0 + peak_bonus),
            post_peak_dt,
            post_peak_half_life_ticks,
            ContainerKind::LooseInPill,
        )
    }
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::super::types::{DecayProfileId, DecayTrack};
    use super::*;

    fn fresh_item(profile: &DecayProfile, initial_qi: f32, created_at_tick: u64) -> Freshness {
        Freshness::new(created_at_tick, initial_qi, profile)
    }

    fn decay_exp_profile(half_life: u64, floor: f32) -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay_exp"),
            formula: DecayFormula::Exponential {
                half_life_ticks: half_life,
            },
            floor_qi: floor,
        }
    }

    fn decay_linear_profile(per_tick: f32, floor: f32) -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay_linear"),
            formula: DecayFormula::Linear {
                decay_per_tick: per_tick,
            },
            floor_qi: floor,
        }
    }

    fn decay_stepwise_profile(floor: f32) -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay_stepwise"),
            formula: DecayFormula::Stepwise,
            floor_qi: floor,
        }
    }

    fn spoil_exp_profile(half_life: u64, threshold: f32) -> DecayProfile {
        DecayProfile::Spoil {
            id: DecayProfileId::new("test_spoil_exp"),
            formula: DecayFormula::Exponential {
                half_life_ticks: half_life,
            },
            spoil_threshold: threshold,
        }
    }

    fn age_profile(peak: u64, bonus: f32, post_half: u64, spoil_th: f32) -> DecayProfile {
        age_profile_with_window(peak, bonus, post_half, spoil_th, 0.1)
    }

    fn age_profile_with_window(
        peak: u64,
        bonus: f32,
        post_half: u64,
        spoil_th: f32,
        window: f32,
    ) -> DecayProfile {
        DecayProfile::Age {
            id: DecayProfileId::new("test_age"),
            peak_at_ticks: peak,
            peak_bonus: bonus,
            peak_window_ratio: window,
            post_peak_half_life_ticks: post_half,
            post_peak_spoil_threshold: spoil_th,
            post_peak_spoil_profile: DecayProfileId::new("test_age_post_spoil"),
        }
    }

    // =========== Exponential Decay ===========

    #[test]
    fn exp_decay_at_creation_returns_initial() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 500);
        let current = compute_current_qi(&f, &p, 500, 1.0);
        assert!((current - 100.0).abs() < 1e-3);
    }

    #[test]
    fn exp_decay_at_one_half_life_returns_half() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 50.0).abs() < 1e-3);
    }

    #[test]
    fn exp_decay_at_two_half_lives_returns_quarter() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 2000, 1.0);
        assert!((current - 25.0).abs() < 1e-3);
    }

    #[test]
    fn dead_zone_multiplier_triples_effective_decay_time() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let normal = compute_current_qi(&f, &p, 1000, zone_multiplier_lookup(0.4));
        let dead_zone = compute_current_qi(&f, &p, 1000, zone_multiplier_lookup(0.0));

        assert!((normal - 50.0).abs() < 1e-3);
        assert!((dead_zone - 12.5).abs() < 1e-3);
        assert_eq!(
            zone_multiplier_lookup(-0.1),
            1.0,
            "negative fields are not ash dead zones"
        );
    }

    #[test]
    fn storage_and_zone_multiplier_compose_transparently() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let combined = combine_storage_and_zone_multiplier(0.5, 0.0);

        assert_eq!(combined, 1.5);
        let current = compute_current_qi(&f, &p, 1000, combined);
        assert!((current - (100.0 * (0.5_f32).powf(1.5))).abs() < 1e-3);
    }

    #[test]
    fn bone_coin_decays_faster_in_summer_than_winter() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);

        let summer = compute_current_qi_with_season(&f, &p, 1000, 1.0, Season::Summer, 7);
        let winter = compute_current_qi_with_season(&f, &p, 1000, 1.0, Season::Winter, 7);

        assert!(summer < winter);
        assert_eq!(season_decay_modifier(Season::Summer, 7), 1.3);
        assert_eq!(season_decay_modifier(Season::Winter, 7), 0.7);
    }

    #[test]
    fn xizhuan_decay_modifier_stays_in_chaotic_band() {
        for season in [Season::SummerToWinter, Season::WinterToSummer] {
            for seed in [0, 1, 42, u64::MAX] {
                let modifier = season_decay_modifier(season, seed);
                assert!((0.8..=1.2).contains(&modifier));
            }
        }
    }

    #[test]
    fn frozen_item_ignores_season_decay_modifier() {
        let p = decay_exp_profile(1000, 0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_since_tick = Some(0);

        let summer = compute_current_qi_with_season(&f, &p, 10_000, 1.0, Season::Summer, 7);
        let winter = compute_current_qi_with_season(&f, &p, 10_000, 1.0, Season::Winter, 7);

        assert_eq!(summer, winter);
        assert!((summer - 100.0).abs() < 1e-3);
    }

    #[test]
    fn exp_decay_floor_qi_clamps_bottom() {
        let p = decay_exp_profile(10, 5.0); // fast decay, floor 5
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 10_000, 1.0);
        assert!((current - 5.0).abs() < 1e-3);
    }

    #[test]
    fn exp_decay_half_life_zero_short_circuits_to_initial() {
        let p = decay_exp_profile(0, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 999_999, 1.0);
        assert!((current - 100.0).abs() < 1e-3);
    }

    // =========== Linear Decay ===========

    #[test]
    fn linear_decay_half_elapsed() {
        let p = decay_linear_profile(0.1, 0.0); // 0.1 per tick
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 500, 1.0);
        assert!((current - 50.0).abs() < 1e-3);
    }

    #[test]
    fn linear_decay_clamps_at_zero_without_floor() {
        let p = decay_linear_profile(1.0, 0.0);
        let f = fresh_item(&p, 10.0, 0);
        let current = compute_current_qi(&f, &p, 1_000_000, 1.0);
        assert!(current >= 0.0);
        assert!(current < 1e-3);
    }

    #[test]
    fn linear_decay_respects_floor() {
        let p = decay_linear_profile(1.0, 3.0);
        let f = fresh_item(&p, 10.0, 0);
        let current = compute_current_qi(&f, &p, 1_000_000, 1.0);
        assert!((current - 3.0).abs() < 1e-3);
    }

    // =========== Stepwise ===========

    #[test]
    fn stepwise_returns_initial_times_multiplier() {
        let p = decay_stepwise_profile(0.0);
        let f = fresh_item(&p, 100.0, 0);

        assert!((compute_current_qi(&f, &p, 10_000, 1.0) - 100.0).abs() < 1e-3);
        assert!((compute_current_qi(&f, &p, 10_000, 0.7) - 70.0).abs() < 1e-3);
        assert!((compute_current_qi(&f, &p, 10_000, 0.3) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn stepwise_ignores_dt_entirely() {
        let p = decay_stepwise_profile(0.0);
        let f = fresh_item(&p, 50.0, 0);
        let a = compute_current_qi(&f, &p, 100, 0.5);
        let b = compute_current_qi(&f, &p, 10_000_000, 0.5);
        assert!((a - b).abs() < 1e-3);
    }

    // =========== storage_multiplier / freezing ===========

    #[test]
    fn storage_multiplier_halves_decay_speed() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        // 玉盒 0.5：到 1000 tick 只衰一半 half_life，current = initial * 0.5^0.5 ≈ 70.71
        let current = compute_current_qi(&f, &p, 1000, 0.5);
        assert!((current - 70.71).abs() < 0.1);
    }

    #[test]
    fn frozen_accumulated_subtracted_from_dt() {
        let p = decay_exp_profile(1000, 0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_accumulated = 500; // 玩家累计 500 tick 在 Freeze 容器里

        // raw_dt = 1000，frozen 500，effective = 500 = 半个 half_life，应 ~70.71
        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 70.71).abs() < 0.1);
    }

    #[test]
    fn frozen_since_tick_inflight_subtracts_from_dt() {
        let p = decay_exp_profile(1000, 0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_since_tick = Some(500); // 当前在 Freeze，进入于 tick 500

        // raw_dt = 1000, inflight_freeze = 500, effective = 500 = half of half_life → ~70.71
        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 70.71).abs() < 0.1);
    }

    #[test]
    fn frozen_multiplier_zero_preserves_initial() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        // 阵法护匣 storage_multiplier=0 → effective_dt = 0 → current = initial
        let current = compute_current_qi(&f, &p, 10_000, 0.0);
        assert!((current - 100.0).abs() < 1e-3);
    }

    #[test]
    fn negative_dt_handled_gracefully() {
        // now_tick < created_at_tick（时空穿越 / clock drift）— 应不 panic，dt 视为 0
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 5000);
        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 100.0).abs() < 1e-3);
    }

    // =========== Spoil ===========

    #[test]
    fn spoil_exp_can_reach_zero() {
        let p = spoil_exp_profile(100, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 1_000_000, 1.0);
        assert!(current < 1e-3);
    }

    #[test]
    fn spoil_track_state_transitions() {
        let p = spoil_exp_profile(1000, 20.0);
        let f = fresh_item(&p, 100.0, 0);

        // 刚创建：Fresh
        assert_eq!(compute_track_state(&f, &p, 0, 1.0), TrackState::Fresh);
        // 1 half_life：current=50，50/100=0.5 → 边界算 Declining
        assert_eq!(
            compute_track_state(&f, &p, 1000, 1.0),
            TrackState::Declining
        );
        // current <= spoil_threshold=20 → Spoiled
        // half_life=1000，要 decay 到 20：0.5^n = 0.2 → n=2.32 → 2320 tick
        assert_eq!(compute_track_state(&f, &p, 3000, 1.0), TrackState::Spoiled);
    }

    // =========== Decay TrackState ===========

    #[test]
    fn decay_track_state_transitions() {
        let p = decay_exp_profile(1000, 5.0);
        let f = fresh_item(&p, 100.0, 0);

        assert_eq!(compute_track_state(&f, &p, 0, 1.0), TrackState::Fresh);
        assert_eq!(
            compute_track_state(&f, &p, 1000, 1.0),
            TrackState::Declining
        );
        // floor_qi=5，要到 <=5：0.5^n ≈ 0.05 → n≈4.32 → 4320 tick
        assert_eq!(compute_track_state(&f, &p, 5000, 1.0), TrackState::Dead);
    }

    // =========== Age ===========

    #[test]
    fn age_pre_peak_linear_bonus() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        // tick 0：current = initial = 100
        assert!((compute_current_qi(&f, &p, 0, 1.0) - 100.0).abs() < 1e-3);
        // tick 500：linear 半程，current = initial * (1 + 0.5 * 0.5) = 100 * 1.25 = 125
        assert!((compute_current_qi(&f, &p, 500, 1.0) - 125.0).abs() < 1e-3);
        // tick 1000（峰值）：current = initial * 1.5 = 150
        assert!((compute_current_qi(&f, &p, 1000, 1.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn age_post_peak_exponential_falloff() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        // peak current = 150
        // post_half_life=500 后：150 * 0.5 = 75
        assert!((compute_current_qi(&f, &p, 1500, 1.0) - 75.0).abs() < 1e-3);
        // post_half_life=1000 后：150 * 0.25 = 37.5
        assert!((compute_current_qi(&f, &p, 2000, 1.0) - 37.5).abs() < 1e-3);
    }

    #[test]
    fn age_track_state_pre_peak_fresh() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        assert_eq!(compute_track_state(&f, &p, 0, 1.0), TrackState::Fresh);
        assert_eq!(compute_track_state(&f, &p, 500, 1.0), TrackState::Fresh);
    }

    #[test]
    fn age_track_state_peaking_window() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        // peak window ±10%：900-1100 tick
        assert_eq!(compute_track_state(&f, &p, 900, 1.0), TrackState::Peaking);
        assert_eq!(compute_track_state(&f, &p, 1000, 1.0), TrackState::Peaking);
        assert_eq!(compute_track_state(&f, &p, 1100, 1.0), TrackState::Peaking);
    }

    #[test]
    fn age_track_state_past_peak() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        // tick 1500：current=75 > spoil_threshold=30 → PastPeak
        assert_eq!(compute_track_state(&f, &p, 1500, 1.0), TrackState::PastPeak);
    }

    #[test]
    fn age_track_state_migrates_to_spoiled() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        // tick 2000：current=37.5 still > 30.0 — so PastPeak, not AgePostPeakSpoiled yet
        assert_eq!(compute_track_state(&f, &p, 2000, 1.0), TrackState::PastPeak);
        // tick 2500：current = 150 * 0.5^3 = 18.75 < 30 → AgePostPeakSpoiled
        assert_eq!(
            compute_track_state(&f, &p, 2500, 1.0),
            TrackState::AgePostPeakSpoiled
        );
    }

    #[test]
    fn age_peak_at_zero_is_instant_initial() {
        let p = age_profile(0, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);
        // peak_at_ticks=0 → short-circuit to initial（避免除 0）
        assert!((compute_current_qi(&f, &p, 5_000, 1.0) - 100.0).abs() < 1e-3);
    }

    // =========== Freshness::new convenience ===========

    #[test]
    fn freshness_new_inherits_track_and_profile_from_profile_arg() {
        let p = decay_exp_profile(1000, 5.0);
        let f = Freshness::new(42, 80.0, &p);
        assert_eq!(f.created_at_tick, 42);
        assert!((f.initial_qi - 80.0).abs() < 1e-3);
        assert_eq!(f.track, DecayTrack::Decay);
        assert_eq!(f.profile.as_str(), "test_decay_exp");
        assert_eq!(f.frozen_accumulated, 0);
        assert!(f.frozen_since_tick.is_none());
    }

    #[test]
    fn freshness_new_for_age_profile_sets_track_age() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = Freshness::new(0, 100.0, &p);
        assert_eq!(f.track, DecayTrack::Age);
    }

    // =========== storage_multiplier negative clamps ===========

    #[test]
    fn negative_storage_multiplier_clamped_to_zero() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);
        let current = compute_current_qi(&f, &p, 10_000, -0.5);
        assert!((current - 100.0).abs() < 1e-3);
    }

    // =========== 组合冻结状态（问题 7） ===========

    #[test]
    fn frozen_accumulated_and_frozen_since_both_subtract() {
        // 玩家历史累积 500 tick 冻结 + 当前从 tick 800 起继续冻结。
        // raw_dt=1000, frozen_accumulated=500, inflight=1000-800=200
        // effective_dt = 1000 - 500 - 200 = 300, half_life=1000 → 300/1000=0.3
        // current = 100 * 0.5^0.3 ≈ 81.225
        let p = decay_exp_profile(1000, 0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_accumulated = 500;
        f.frozen_since_tick = Some(800);

        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 81.225).abs() < 0.1);
    }

    #[test]
    fn frozen_cannot_go_negative_when_over_subtracted() {
        // 极端 malformed 数据：frozen_accumulated > raw_dt
        let p = decay_exp_profile(1000, 0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_accumulated = 10_000_000;

        // saturating_sub 保护：effective_dt = 0 → current = initial
        let current = compute_current_qi(&f, &p, 1000, 1.0);
        assert!((current - 100.0).abs() < 1e-3);
    }

    // =========== Stepwise + 冻结交互（问题 7） ===========

    #[test]
    fn stepwise_ignores_frozen_state() {
        let p = decay_stepwise_profile(0.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.frozen_accumulated = 500;
        f.frozen_since_tick = Some(700);

        // Stepwise 不用 dt，只看 multiplier
        let current = compute_current_qi(&f, &p, 1000, 0.7);
        assert!((current - 70.0).abs() < 1e-3);
    }

    // =========== 峰值窗口 ratio 参数化（问题 2） ===========

    #[test]
    fn age_peaking_window_narrow_5pct() {
        let p = age_profile_with_window(1000, 0.5, 500, 30.0, 0.05);
        let f = fresh_item(&p, 100.0, 0);
        // 窗口 950-1050
        assert_eq!(compute_track_state(&f, &p, 949, 1.0), TrackState::Fresh);
        assert_eq!(compute_track_state(&f, &p, 950, 1.0), TrackState::Peaking);
        assert_eq!(compute_track_state(&f, &p, 1050, 1.0), TrackState::Peaking);
        assert_eq!(compute_track_state(&f, &p, 1051, 1.0), TrackState::PastPeak);
    }

    #[test]
    fn age_peaking_window_wide_20pct() {
        let p = age_profile_with_window(1000, 0.5, 500, 30.0, 0.2);
        let f = fresh_item(&p, 100.0, 0);
        // 窗口 800-1200
        assert_eq!(compute_track_state(&f, &p, 800, 1.0), TrackState::Peaking);
        assert_eq!(compute_track_state(&f, &p, 1200, 1.0), TrackState::Peaking);
    }

    // =========== Decay/Spoil headroom ratio（问题 3） ===========

    #[test]
    fn decay_declining_uses_headroom_not_raw_initial() {
        // initial=10, floor=5, headroom=5。 current=8 → remaining=3 / headroom=5 = 0.6 → Fresh
        // current=6 → remaining=1 / 5 = 0.2 → Declining（原 raw 0.6 比率公式会判 Fresh）
        let p = decay_exp_profile(10_000, 5.0);
        let mut f = fresh_item(&p, 10.0, 0);

        f.initial_qi = 10.0;
        // 手工设置 created_at 让 current 达到指定值
        // At half_life=10000, dt 使 current=6：0.5^n = 0.6 → n=0.737 → dt≈7370
        let state_at_6 = compute_track_state(&f, &p, 7370, 1.0);
        // current ≈ 6 (initial 10, floor 5, headroom 5, remaining 1)
        assert_eq!(
            state_at_6,
            TrackState::Declining,
            "current≈6 near floor should be Declining not Fresh"
        );
    }

    #[test]
    fn spoil_declining_uses_headroom() {
        // initial=50, spoil_threshold=30, headroom=20
        // current=45 → remaining=15/20=0.75 → Fresh
        // current=35 → remaining=5/20=0.25 → Declining
        let p = spoil_exp_profile(10_000, 30.0);
        let f = fresh_item(&p, 50.0, 0);

        // At dt where current=35：0.5^n = 35/50 = 0.7 → n=0.515 → dt≈5146
        let state = compute_track_state(&f, &p, 5146, 1.0);
        assert_eq!(state, TrackState::Declining);
    }

    // =========== Age 迁移顺序修正（问题 5） ===========

    #[test]
    fn malformed_age_initial_below_threshold_is_fresh_not_spoiled() {
        // malformed config: initial=20 但 post_peak_spoil_threshold=30
        // 旧代码：current 起始就 <= 30 → AgePostPeakSpoiled（错误）
        // 修正后：未过 peak 不触发 Spoiled 迁移 → Fresh
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 20.0, 0);

        assert_eq!(
            compute_track_state(&f, &p, 0, 1.0),
            TrackState::Fresh,
            "malformed config: initial < threshold pre-peak should be Fresh, not AgePostPeakSpoiled"
        );
        // 到峰值时：current = 20 * 1.5 = 30 > threshold 30 边界 → Peaking
        assert_eq!(compute_track_state(&f, &p, 1000, 1.0), TrackState::Peaking);
    }

    // =========== Linear f64 精度保护（问题 9） ===========

    #[test]
    fn linear_long_range_decay_precision() {
        // 骨币场景：initial=100, ~1y=6.3e8 ticks @ 20 TPS
        // decay_per_tick = 100 / 6.3e8 ≈ 1.587e-7（1y 完全衰减）
        let p = decay_linear_profile(1.0e-7, 0.0);
        let f = fresh_item(&p, 100.0, 0);

        // 半年 ≈ 3.15e8 tick
        let half_year = compute_current_qi(&f, &p, 315_000_000, 1.0);
        // 期望 100 - 1e-7 * 3.15e8 = 100 - 31.5 = 68.5
        assert!(
            (half_year - 68.5).abs() < 0.01,
            "linear half-year: expected ~68.5, got {half_year}"
        );
    }

    // =========== Serde roundtrip（问题 6） ===========

    #[test]
    fn freshness_serde_roundtrip() {
        let p = decay_exp_profile(1000, 5.0);
        let mut f = Freshness::new(100, 80.0, &p);
        f.frozen_accumulated = 42;
        f.frozen_since_tick = Some(200);

        let json = serde_json::to_string(&f).expect("serialize");
        let decoded: Freshness = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.created_at_tick, 100);
        assert!((decoded.initial_qi - 80.0).abs() < 1e-3);
        assert_eq!(decoded.track, DecayTrack::Decay);
        assert_eq!(decoded.profile.as_str(), "test_decay_exp");
        assert_eq!(decoded.frozen_accumulated, 42);
        assert_eq!(decoded.frozen_since_tick, Some(200));
    }

    #[test]
    fn freshness_serde_legacy_missing_frozen_fields_defaults() {
        // v1 初版 NBT（缺 frozen_accumulated / frozen_since_tick）应能正确 deserialize。
        let legacy_json = serde_json::json!({
            "created_at_tick": 100,
            "initial_qi": 80.0,
            "track": "Decay",
            "profile": "legacy_profile",
        });

        let decoded: Freshness =
            serde_json::from_value(legacy_json).expect("legacy deserialize with #[serde(default)]");
        assert_eq!(decoded.frozen_accumulated, 0);
        assert!(decoded.frozen_since_tick.is_none());
    }

    #[test]
    fn decay_profile_serde_roundtrip_all_three_variants() {
        let decay = decay_exp_profile(1000, 5.0);
        let spoil = spoil_exp_profile(500, 20.0);
        let age = age_profile_with_window(1000, 0.5, 500, 30.0, 0.1);

        for p in [decay, spoil, age] {
            let j = serde_json::to_string(&p).expect("ser");
            let back: DecayProfile = serde_json::from_str(&j).expect("de");
            assert_eq!(back, p);
        }
    }

    // =========== DecayProfile::validate（问题 10） ===========

    #[test]
    fn validate_accepts_valid_profiles() {
        assert!(decay_exp_profile(1000, 5.0).validate().is_ok());
        assert!(spoil_exp_profile(500, 20.0).validate().is_ok());
        assert!(age_profile(1000, 0.5, 500, 30.0).validate().is_ok());
        assert!(decay_stepwise_profile(0.0).validate().is_ok());
    }

    #[test]
    fn validate_rejects_age_zero_peak() {
        let p = age_profile(0, 0.5, 500, 30.0);
        let err = p.validate().unwrap_err();
        assert!(err.contains("peak_at_ticks"));
    }

    #[test]
    fn validate_rejects_age_negative_peak_bonus() {
        let p = age_profile(1000, -0.1, 500, 30.0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn validate_rejects_age_window_ratio_out_of_range() {
        let over = age_profile_with_window(1000, 0.5, 500, 30.0, 1.5);
        let neg = age_profile_with_window(1000, 0.5, 500, 30.0, -0.1);
        assert!(over.validate().is_err());
        assert!(neg.validate().is_err());
    }

    #[test]
    fn validate_rejects_decay_negative_floor() {
        let p = DecayProfile::Decay {
            id: DecayProfileId::new("bad"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: -1.0,
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn validate_rejects_linear_negative_rate() {
        let p = DecayProfile::Decay {
            id: DecayProfileId::new("bad"),
            formula: DecayFormula::Linear {
                decay_per_tick: -0.1,
            },
            floor_qi: 0.0,
        };
        assert!(p.validate().is_err());
    }

    // =========== Codex P2-1: compute_track_state zero-peak guard ===========

    #[test]
    fn age_zero_peak_track_state_stays_fresh() {
        // Regression for Codex review P2-1: compute_current_qi has peak_at_ticks==0 guard
        // (returns initial), but compute_track_state lacked matching guard and would compute
        // peak_hi=0, classifying any future tick as PastPeak — inconsistent with current_qi
        // behavior. Guard now explicitly returns Fresh.
        let p = age_profile(0, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0, 0);

        assert_eq!(compute_track_state(&f, &p, 0, 1.0), TrackState::Fresh);
        assert_eq!(compute_track_state(&f, &p, 1000, 1.0), TrackState::Fresh);
        assert_eq!(
            compute_track_state(&f, &p, 10_000_000, 1.0),
            TrackState::Fresh
        );
    }

    // =========== Codex P2-2: Spoil/Age 阈值严格 `<` ===========

    #[test]
    fn spoil_exactly_at_threshold_is_not_spoiled() {
        // Regression for Codex P2-2: plan §6.3 spec says `current_qi < spoil_threshold`,
        // boundary (current == threshold) should stay Fresh/Declining, not trigger Spoiled.
        let p = spoil_exp_profile(1000, 50.0);
        let mut f = fresh_item(&p, 100.0, 0);
        f.initial_qi = 100.0;

        // 构造 current 恰好 = 50.0：0.5^n = 0.5 → n=1 → dt=1000
        let state = compute_track_state(&f, &p, 1000, 1.0);
        assert_ne!(
            state,
            TrackState::Spoiled,
            "at exactly spoil_threshold should NOT trigger Spoiled (plan §6.3 strict `<`)"
        );
    }

    #[test]
    fn age_exactly_at_post_peak_spoil_threshold_not_migrated() {
        // 同上，Age 路径 post_peak_spoil_threshold 也应严格 `<`。
        // peak_at=1000, initial=100, bonus=0.5, peak_current=150, post_half=500,
        // post_peak_spoil_threshold=75.0（正好是 1 post-half-life 值：150 * 0.5 = 75）
        let p = age_profile(1000, 0.5, 500, 75.0);
        let f = fresh_item(&p, 100.0, 0);

        // tick 1500: post_peak_dt=500, current = 150 * 0.5 = 75.0 (exactly threshold)
        let state = compute_track_state(&f, &p, 1500, 1.0);
        assert_ne!(
            state,
            TrackState::AgePostPeakSpoiled,
            "at exactly post_peak_spoil_threshold should NOT trigger migration (plan §6.3 strict `<`)"
        );
        assert_eq!(state, TrackState::PastPeak);
    }

    #[test]
    fn validate_rejects_nan_parameters() {
        let nan_decay = DecayProfile::Decay {
            id: DecayProfileId::new("bad"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: f32::NAN,
        };
        assert!(nan_decay.validate().is_err());
    }

    #[test]
    fn exponential_decay_uses_qi_physics_excretion_calibration() {
        let p = decay_exp_profile(1000, 0.0);
        let f = fresh_item(&p, 100.0, 0);

        let current = compute_current_qi(&f, &p, 1000, 1.0);

        assert!(
            (current - 50.0).abs() < 1e-3,
            "qi_excretion calibration must preserve existing half-life curves, got {current}"
        );
    }

    #[test]
    fn profile_container_kind_maps_production_families_to_qi_physics_containers() {
        let ling_shi = DecayProfile::Decay {
            id: DecayProfileId::new("ling_shi_fan_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };
        let bone = DecayProfile::Decay {
            id: DecayProfileId::new("bone_coin_40_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };
        let wood = DecayProfile::Decay {
            id: DecayProfileId::new("ling_mu_gun_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };

        assert_eq!(
            profile_container_kind(&ling_shi),
            ContainerKind::SealedAncientRelic
        );
        assert_eq!(profile_container_kind(&bone), ContainerKind::SealedInBone);
        assert_eq!(
            profile_container_kind(&wood),
            ContainerKind::WieldedInWeapon
        );
    }
}
