//! worldview 锚定的真元物理常量。

/// worldview §四：距离基础衰减，正典 0.03 / block。
pub const QI_DECAY_PER_BLOCK: f64 = 0.03;
/// worldview §二：离体真元被末法残土分解的基础速率。
pub const QI_AMBIENT_EXCRETION_PER_SEC: f64 = 0.001;
/// worldview §四：异体排斥基础率。
pub const QI_EXCRETION_BASE: f64 = 0.30;
/// worldview §四：声学激发阈值。
pub const QI_ACOUSTIC_THRESHOLD: f64 = 0.40;
/// worldview §四：涡流 1/r^2 系数。
pub const QI_NEGATIVE_FIELD_K: f64 = 1.0;
/// worldview §四：涡流反向获利上限。
pub const QI_DRAIN_CLAMP: f64 = 0.50;
/// plan-woliu-v1：旧涡流线性抽吸归一阈值；1/r^2 迁移留给 P2-1。
pub const QI_WOLIU_VORTEX_THEORETICAL_LIMIT_DELTA: f32 = 0.8;
/// worldview §九：骨币半衰参考，约 13 天。
pub const QI_HALFLIFE_REFERENCE_DAYS: f64 = 13.0;
/// plan-shelflife-v1：死域保质期衰减倍率，保持旧曲线三倍加速。
pub const QI_SHELFLIFE_DEAD_ZONE_MULTIPLIER: f32 = 3.0;
/// worldview §四：防御维持基线。
pub const QI_MAINTENANCE_IDLE: f64 = 1.0;
/// worldview §十六：末法残土抽真元强度。
pub const QI_TSY_DRAIN_FACTOR: f64 = 0.5;
/// worldview §十六：末法残土非线性指数。
pub const QI_TSY_DRAIN_NONLINEAR_EXPONENT: f64 = 1.5;
/// plan-tsy-zone-v1：TSY 抽取基准真元池。
pub const QI_TSY_REFERENCE_POOL: f64 = 100.0;
/// plan-tsy-zone-v1：TSY 抽取基准速率（点 / tick）。
pub const QI_TSY_BASE_DRAIN_PER_TICK: f64 = 0.5;
/// plan-tsy-container-v1：搜刮中主动暴露的 TSY 抽取放大因子。
pub const QI_TSY_SEARCH_EXPOSURE_FACTOR: f64 = 1.5;
/// worldview §十七：中性节律。
pub const QI_RHYTHM_NEUTRAL: f64 = 1.0;
/// worldview §十七：活跃节律。
pub const QI_RHYTHM_ACTIVE: f64 = 1.2;
/// worldview §十七：汐转波动范围。
pub const QI_RHYTHM_TURBULENT_RANGE: (f64, f64) = (0.7, 1.5);
/// worldview §十：全服灵气预算默认值；生产值由 server config 初始化。
pub const DEFAULT_SPIRIT_QI_TOTAL: f64 = 100.0;
/// worldview §十：天道时代衰减下限。
pub const QI_TIANDAO_DECAY_PER_ERA_MIN: f64 = 0.01;
/// worldview §十：天道时代衰减上限。
pub const QI_TIANDAO_DECAY_PER_ERA_MAX: f64 = 0.03;
/// worldview §十一：灵物密度阈值。
pub const QI_DENSITY_GAZE_THRESHOLD: f64 = 0.85;
/// worldview §九/§十一：区域饥饿阈值。
pub const QI_REGION_STARVATION_THRESHOLD: f64 = 0.1;
/// plan-cultivation-v1：修炼吸纳速率系数。
pub const QI_CULTIVATION_REGEN_RATE: f64 = 0.01;
/// plan-cultivation-v1：1.0 zone 浓度可支撑的玩家真元点数。
pub const QI_ZONE_UNIT_CAPACITY: f64 = 50.0;
/// player gather：采集动作默认真元奖励，以 zone qi 对冲供给。
pub const QI_GATHER_REWARD: f64 = 14.0;
/// plan-lingtian-v1：偷灵注入操作者比例。
pub const LINGTIAN_DRAIN_PLAYER_RATIO: f32 = 0.8;
/// plan-lingtian-v1：偷灵散逸回 zone 比例。
pub const LINGTIAN_DRAIN_ZONE_RATIO: f32 = 0.2;
/// plan-lingtian-v1：plot 灵气不足时从环境场漏吸的比例。
pub const QI_LINGTIAN_AMBIENT_LEAK_RATIO: f32 = 0.2;
/// plan-zhenmai-v1：截脉防御窗口基础时长。
pub const QI_ZHENMAI_PREP_WINDOW_MS: u32 = 1000;
/// plan-zhenmai-v1：截脉污染残留倍率。
pub const QI_ZHENMAI_CONTAM_RESIDUAL_MULTIPLIER: f64 = 0.2;
/// plan-zhenmai-v1：截脉震荡基础严重度。
pub const QI_ZHENMAI_CONCUSSION_BASE_SEVERITY: f32 = 0.3;
/// plan-zhenmai-v1：截脉震荡流血速率。
pub const QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC: f32 = 0.0;
/// plan-zhenmai-v1：招架恢复 tick。
pub const QI_ZHENMAI_PARRY_RECOVERY_TICKS: u64 = 10;
/// plan-zhenmai-v1：招架恢复期间移动速度倍率。
pub const QI_ZHENMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER: f32 = 0.7;
/// 数值断言默认容忍度。
pub const QI_EPSILON: f64 = 1e-6;
