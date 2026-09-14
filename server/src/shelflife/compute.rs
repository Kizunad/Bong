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
