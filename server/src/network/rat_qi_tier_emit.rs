//! 噬元鼠吸元档位 S2C CustomPayload —— plan-devour-rat-model P2
//!
//! 噬元鼠靠咬人吸真元（`RatBlackboard.drained_qi` 镜像 `npc:rat:<id>` ledger 余额）。
//! 本模块把该储量离散成 3 档下发给客户端，客户端据此换贴图变体
//! （`devour_rat_q{0,1,2}.png`：尾脊蓝填充 0 / 半 / 全）并挂 emissive glow 层。
//!
//! channel：`bong:rat_qi_tier`（单向 S2C，join + 周期全量 sync）
//!
//! payload JSON：
//! ```json
//! {
//!   "v": 1,
//!   "type": "rat_qi_tier",
//!   "entries": [{"id": 42, "tier": 2}, {"id": 77, "tier": 1}]
//! }
//! ```
//!
//! **缺省即 0 档**：`tier == 0` 的鼠**不进 wire**（绝大多数鼠一口没咬过），client 端查不到
//! 就是 0 档。这既把 payload 压到最小（鼠患动辄成群，全量带上会撑爆
//! `MAX_PAYLOAD_BYTES`），也让"全量替换"语义天然覆盖降档：只要某只鼠不在新名单里，
//! client 立刻把它当 0 档。
//!
//! # 守恒律
//!
//! 本模块**只读** `RatBlackboard.drained_qi`，不写 `WorldQiAccount` / `Zone.spirit_qi` /
//! `Cultivation.qi_current`，不 emit `QiTransfer`。档位阈值是**纯展示分档**（把一个已存在
//! 的真元储量映射成 0/1/2 三个贴图变体），不是真元物理常数——不参与任何衰减 / 转移 /
//! 生成计算，故不属于 `qi_physics::constants` 的辖域。鼠储量的入账在
//! `combat::rat_bite`，出账（死亡 / 超距回收归还 zone）在 `fauna::rat_phase`。

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{ident, Added, Client, Position, Query, Res, ViewDistance, With};

use crate::cultivation::tick::CultivationClock;
use crate::network::disguise_sync::sync_radius_blocks;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

/// 下发 `rat_qi_tier` 的节流间隔（tick）。
///
/// 比伪装名单同步（`SPIDER_DISGUISE_SYNC_INTERVAL_TICKS` = 40）快一倍：档位变化是玩家
/// 被咬后的**即时反馈**（尾脊亮起来），1s 内跟上即可，无需再加一条增量 channel。
pub const RAT_QI_TIER_SYNC_INTERVAL_TICKS: u64 = 20;

/// 1 档（尾脊半亮）门槛：吸到 ≥ 1 点真元。`RatBiteEvent.qi_steal` 常规为 1，
/// 即"咬中一口就开始泛蓝"。
pub const RAT_QI_TIER_1_THRESHOLD: f64 = 1.0;

/// 2 档（尾脊全亮）门槛：吸到 ≥ 5 点真元 —— 常规咬击 5 口，肉眼可辨的"吃饱了"状态。
pub const RAT_QI_TIER_2_THRESHOLD: f64 = 5.0;

/// 档位上限；client `FaunaModel.devourRatTexture` 同值 clamp（贴图只有 q0/q1/q2）。
pub const RAT_QI_TIER_MAX: u8 = 2;

/// 吸元储量 → 展示档位。
///
/// 负值与**全部**非有限值（NaN / +inf / -inf）一律归 0 档：wire 上只允许
/// 0..=`RAT_QI_TIER_MAX`，不能让脏数值变成客户端查不到的贴图下标。
///
/// `+inf` 也归 0 而非饱和到满档：`drained_qi` 镜像的是被 `SPIRIT_QI_TOTAL` 封顶的账本余额，
/// 物理上到不了 inf，出现即账本损坏——脏数据不该被奖励成最亮的表现，且 NaN 本就比较不出来，
/// 给 `+inf` 开特例只会让这条规则多一个记不住的分叉。
pub fn rat_qi_tier(drained_qi: f64) -> u8 {
    if !drained_qi.is_finite() || drained_qi < RAT_QI_TIER_1_THRESHOLD {
        return 0;
    }
    if drained_qi < RAT_QI_TIER_2_THRESHOLD {
        1
    } else {
        RAT_QI_TIER_MAX
    }
}

/// 单只鼠的档位条目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatQiTierEntry {
    /// Valence `EntityId`（MC 协议 entity id），client 用 `world.getEntityById` 定位。
    pub id: i32,
    /// 0..=[`RAT_QI_TIER_MAX`]。
    pub tier: u8,
}

/// `bong:rat_qi_tier` S2C payload。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatQiTierS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    /// 仅含 `tier > 0` 的鼠；client 端全量替换，缺席 = 0 档。
    pub entries: Vec<RatQiTierEntry>,
}

impl RatQiTierS2c {
    pub fn sync(entries: Vec<RatQiTierEntry>) -> Self {
        Self {
            v: 1,
            ty: "rat_qi_tier".to_string(),
            entries,
        }
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

type RatTierQuery<'w, 's> =
    Query<'w, 's, (&'static EntityId, &'static Position, &'static RatBlackboard), With<NpcMarker>>;

/// 纯逻辑核：(entity_id, 坐标, drained_qi) 流 → 只保留 `tier > 0` 的候选。
///
/// 与 ECS 解耦以便单测锁"0 档不进 wire"这条 payload 收敛契约。
fn tiered_candidates(
    entries: impl Iterator<Item = (i32, [f64; 3], f64)>,
) -> Vec<(i32, [f64; 3], u8)> {
    entries
        .filter_map(|(id, pos, drained)| {
            let tier = rat_qi_tier(drained);
            (tier > 0).then_some((id, pos, tier))
        })
        .collect()
}

/// 视距过滤：XZ Chebyshev 半径内的候选 → wire 条目，**按到 client 的距离升序**。
///
/// 复用 `disguise_sync::sync_radius_blocks` 的半径推导（逐 client 从 `ViewDistance` 算），
/// 与伪装名单同一套口径；这里额外携带 tier，故不能直接调 `ids_visible_to_client`。
///
/// 排序是为了让 [`fit_entries_within_payload_limit`] 在超限时能"就近保真"——丢也先丢远处的。
fn entries_within_radius(
    candidates: &[(i32, [f64; 3], u8)],
    center: [f64; 3],
    radius: f64,
) -> Vec<RatQiTierEntry> {
    let mut visible: Vec<(f64, RatQiTierEntry)> = candidates
        .iter()
        .filter(|(_, pos, _)| {
            (pos[0] - center[0]).abs() <= radius && (pos[2] - center[2]).abs() <= radius
        })
        .map(|(id, pos, tier)| {
            let dx = pos[0] - center[0];
            let dz = pos[2] - center[2];
            (
                dx * dx + dz * dz,
                RatQiTierEntry {
                    id: *id,
                    tier: *tier,
                },
            )
        })
        .collect();
    // 距离相等时按 id 兜底排序，保证输出稳定（同一批输入恒得同一条 wire）。
    visible.sort_by(|(da, a), (db, b)| {
        da.partial_cmp(db)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    visible.into_iter().map(|(_, entry)| entry).collect()
}

/// 把条目裁到能装进 `MAX_PAYLOAD_BYTES` 为止（从**末尾**即最远处开始丢）。
///
/// 为什么不是"超限就整包丢弃"：本模块的降档语义完全依赖「全量替换 + 空表 keepalive」。
/// 整包丢弃会让客户端的档位表**永不更新**——已死 / 已离开视野的鼠会一直亮着上一次的档位，
/// 而且下个周期同样超限，没有自愈路径。裁到装得下则至少近处的鼠保持正确，远处的鼠退化成
/// 0 档（缺席即 0 档），是可自愈的降级。
///
/// 返回 `(裁剪后的条目, 被丢弃的条目数)`。
///
/// 用二分找最长可装下的前缀（payload 字节数对条目数单调递增），避免逐个 pop 的
/// O(n²) 序列化；正常路径（不超限）只多花一次序列化探测。
fn fit_entries_within_payload_limit(entries: Vec<RatQiTierEntry>) -> (Vec<RatQiTierEntry>, usize) {
    if RatQiTierS2c::sync(entries.clone())
        .to_json_bytes_checked()
        .is_ok()
    {
        return (entries, 0);
    }
    let total = entries.len();
    // 不变式：`lo` 恒可装下（空表必可装下），`hi` 恒装不下。
    let (mut lo, mut hi) = (0usize, total);
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        if RatQiTierS2c::sync(entries[..mid].to_vec())
            .to_json_bytes_checked()
            .is_ok()
        {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let mut kept = entries;
    kept.truncate(lo);
    (kept, total - lo)
}

fn collect_tiered_rats(rats: &RatTierQuery<'_, '_>) -> Vec<(i32, [f64; 3], u8)> {
    tiered_candidates(rats.iter().map(|(eid, pos, blackboard)| {
        let p = pos.get();
        (eid.get(), [p.x, p.y, p.z], blackboard.drained_qi)
    }))
}

fn send_tier_payload(
    client: &mut Client,
    client_pos: &Position,
    view_distance: &ViewDistance,
    candidates: &[(i32, [f64; 3], u8)],
) {
    let p = client_pos.get();
    let entries = entries_within_radius(
        candidates,
        [p.x, p.y, p.z],
        sync_radius_blocks(view_distance.get()),
    );
    let (entries, dropped) = fit_entries_within_payload_limit(entries);
    if dropped > 0 {
        tracing::warn!(
            "[bong][rat_qi_tier] payload oversize, dropped {dropped} farthest entries (kept {})",
            entries.len()
        );
    }
    let payload = RatQiTierS2c::sync(entries);
    let Ok(bytes) = payload.to_json_bytes_checked() else {
        // 走到这里说明连空表都装不下（理论不可能），保持"不发"而非 panic。
        tracing::error!("[bong][rat_qi_tier] payload oversize even after truncation, skip");
        return;
    };
    client.send_custom_payload(ident!("bong:rat_qi_tier"), &bytes);
}

/// 新玩家连接时推送其视野内的吸元档位名单。
pub fn on_player_join_send_rat_qi_tiers(
    mut new_clients: Query<(&mut Client, &Position, &ViewDistance), Added<Client>>,
    rats: RatTierQuery<'_, '_>,
) {
    if new_clients.is_empty() {
        return;
    }
    let candidates = collect_tiered_rats(&rats);
    for (mut client, client_pos, view_distance) in &mut new_clients {
        send_tier_payload(&mut client, client_pos, view_distance, &candidates);
    }
}

/// 周期性全量同步视野内的吸元档位名单。
///
/// 空表照发：client 端是全量替换语义，空表 keepalive 负责把"鼠已死 / 已离开视野 /
/// 储量已归还 zone 降回 0 档"的 stale 条目清掉。
pub fn periodic_rat_qi_tier_sync_system(
    clock: Res<CultivationClock>,
    mut clients: Query<(&mut Client, &Position, &ViewDistance)>,
    rats: RatTierQuery<'_, '_>,
) {
    if !clock.tick.is_multiple_of(RAT_QI_TIER_SYNC_INTERVAL_TICKS) {
        return;
    }
    let candidates = collect_tiered_rats(&rats);
    for (mut client, client_pos, view_distance) in &mut clients {
        send_tier_payload(&mut client, client_pos, view_distance, &candidates);
    }
}

pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::Update;
    app.add_systems(
        Update,
        (
            on_player_join_send_rat_qi_tiers,
            periodic_rat_qi_tier_sync_system,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 接线核验（防功能孤岛）────────────────────────────────────────────────

    /// 取源码的**生产半区**（砍掉 inline `#[cfg(test)] mod tests {` 之后的内容 + 剔纯注释行）。
    /// 详见 `network::rat_av_trigger` 里同名 helper 的注释——不剔注释的话
    /// `// rat_qi_tier_emit::register(app);` 这种注释掉的调用也能让 pin 通过。
    fn production_source(src: &str) -> String {
        let head = src
            .split_once("#[cfg(test)]\nmod tests {")
            .map(|(head, _)| head)
            // 切分串对 rustfmt 输出形态敏感（属性与 mod 之间恰一个 \n、`{` 同行）。落空时
            // 直接 panic 把失败点钉在真正的原因上，而不是退化成"返回全文 → pin 恒真"、
            // 让同伴元测试以"生产半区出现多次"这种误导性信息撞红。
            .unwrap_or_else(|| {
                panic!(
                    "production_source 未找到 inline 测试模块起点 `#[cfg(test)]\\nmod tests {{`；\
                     被审文件的形态变了（属性与 mod 间多了空行/注释？），切分逻辑需同步更新"
                )
            });
        head.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `register` 写了却没被 `network::register` 调用 = 永不运行的孤岛：档位永不下发，
    /// 客户端所有鼠恒渲染 q0，而本文件的纯函数单测照样全绿。
    ///
    /// ECS 系统本身起不了真 app（需要 valence 全量插件 + client），故退一步在**源码层**
    /// 钉住调用点存在（对齐 client 侧 `FaunaEmissiveGlowWiringTest` 的字节码接线核验思路）。
    #[test]
    fn register_is_wired_into_network_register() {
        let network_mod = production_source(include_str!("mod.rs"));
        assert!(
            network_mod.contains("rat_qi_tier_emit::register(app)"),
            "network::register 必须调用 rat_qi_tier_emit::register(app)，否则档位下发系统\
             永不运行——客户端所有噬元鼠恒渲染 q0，而本模块单测仍会全绿"
        );
        assert!(
            network_mod.contains("pub mod rat_qi_tier_emit;"),
            "rat_qi_tier_emit 必须在 network 下声明为模块"
        );
    }

    /// 元测试：证明上面那条 pin 不是恒真的（删掉生产注册行就该失去命中）。
    #[test]
    fn wiring_pin_actually_fails_when_production_registration_is_removed() {
        let network_mod = production_source(include_str!("mod.rs"));
        let mutated = network_mod.replacen("    rat_qi_tier_emit::register(app);\n", "", 1);
        assert!(
            !mutated.contains("rat_qi_tier_emit::register(app)"),
            "生产半区里 rat_qi_tier_emit::register(app) 应当只出现一次；删掉后仍命中 = \
             接线 pin 恒真、挡不住孤岛"
        );
    }

    // ── 档位阈值（含边界 off-by-one 与脏数值）────────────────────────────────

    #[test]
    fn tier_zero_below_first_threshold() {
        for qi in [0.0, 0.5, RAT_QI_TIER_1_THRESHOLD - 1e-9] {
            assert_eq!(
                rat_qi_tier(qi),
                0,
                "drained_qi={qi} < {RAT_QI_TIER_1_THRESHOLD} 应为 0 档（尾脊全黑）"
            );
        }
    }

    #[test]
    fn tier_one_at_and_above_first_threshold() {
        for qi in [
            RAT_QI_TIER_1_THRESHOLD,
            RAT_QI_TIER_1_THRESHOLD + 1e-9,
            RAT_QI_TIER_2_THRESHOLD - 1e-9,
        ] {
            assert_eq!(
                rat_qi_tier(qi),
                1,
                "drained_qi={qi} ∈ [{RAT_QI_TIER_1_THRESHOLD}, {RAT_QI_TIER_2_THRESHOLD}) \
                 应为 1 档（尾脊半亮）"
            );
        }
    }

    #[test]
    fn tier_two_at_and_above_second_threshold() {
        for qi in [RAT_QI_TIER_2_THRESHOLD, 50.0, f64::MAX] {
            assert_eq!(
                rat_qi_tier(qi),
                RAT_QI_TIER_MAX,
                "drained_qi={qi} ≥ {RAT_QI_TIER_2_THRESHOLD} 应为 2 档（尾脊全亮）"
            );
        }
    }

    /// 非有限值一律归 0 档 —— 包括 `+inf`。
    ///
    /// `+inf` 数值上确实落在「大于二档阈值」一侧，但它和 NaN 一样只可能来自**账本被写坏**，
    /// 不是「吸得特别多」。选择让所有非有限值走同一条保守分支（0 档），而不是给 `+inf` 开
    /// 一个「饱和到满档」的特例：脏数据不该被奖励成最亮的表现，且 NaN 无论如何比较不出来，
    /// 特例化只会让这条规则多一个记不住的分叉。
    #[test]
    fn tier_zero_for_negative_and_all_non_finite() {
        for qi in [-1.0, -f64::MAX, f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
            assert_eq!(
                rat_qi_tier(qi),
                0,
                "脏储量 {qi} 必须归 0 档：非有限值只可能来自账本损坏，不能让它变成\
                 客户端查不到的贴图下标、也不该被奖励成满档发光"
            );
        }
    }

    // ── wire 格式 pin（client RatQiTierHandler 逐字对齐）──────────────────────

    #[test]
    fn wire_format_pin() {
        let payload = RatQiTierS2c::sync(vec![
            RatQiTierEntry { id: 42, tier: 2 },
            RatQiTierEntry { id: 77, tier: 1 },
        ]);
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["v"], 1, "版本号必须为 1（client 端 v!=1 直接拒收）");
        assert_eq!(
            v["type"], "rat_qi_tier",
            "type wire 名必须稳定，client handler 按它做 expectedType 校验"
        );
        assert_eq!(
            v["entries"],
            serde_json::json!([{"id": 42, "tier": 2}, {"id": 77, "tier": 1}]),
            "entries 必须是 [{{id, tier}}] 对象数组，字段名逐字对齐 client 解析"
        );
    }

    #[test]
    fn type_field_is_renamed_on_wire() {
        let json = serde_json::to_string(&RatQiTierS2c::sync(vec![])).unwrap();
        assert!(
            json.contains("\"type\":\"rat_qi_tier\""),
            "type 字段 wire name 必须为 rat_qi_tier（serde rename），实际 {json}"
        );
    }

    #[test]
    fn empty_entries_serialize_as_empty_array() {
        let json = serde_json::to_string(&RatQiTierS2c::sync(vec![])).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["entries"].as_array().is_some_and(|a| a.is_empty()),
            "空名单必须序列化为 []（keepalive 全量替换语义靠它清 stale），实际 {:?}",
            v["entries"]
        );
    }

    #[test]
    fn payload_roundtrip() {
        let original = RatQiTierS2c::sync(vec![
            RatQiTierEntry { id: 1, tier: 1 },
            RatQiTierEntry { id: 2, tier: 2 },
        ]);
        let json = serde_json::to_string(&original).unwrap();
        let decoded: RatQiTierS2c = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded, "RatQiTierS2c 序列化往返必须等价");
    }

    #[test]
    fn oversize_payload_is_rejected() {
        // 单条 ~24 字节，MAX_PAYLOAD_BYTES 上限内塞不下的量必须报 Oversize 而非静默截断。
        let entries: Vec<RatQiTierEntry> = (0..(MAX_PAYLOAD_BYTES as i32))
            .map(|id| RatQiTierEntry { id, tier: 2 })
            .collect();
        let err = RatQiTierS2c::sync(entries)
            .to_json_bytes_checked()
            .expect_err("超大名单必须返回 Oversize");
        assert!(
            matches!(err, ServerDataBuildError::Oversize { .. }),
            "应为 Oversize 错误，实际 {err:?}"
        );
    }

    // ── tiered_candidates：0 档收敛 ──────────────────────────────────────────

    #[test]
    fn tiered_candidates_drops_tier_zero_rats() {
        let got = tiered_candidates(
            vec![
                (1, [0.0, 64.0, 0.0], 0.0),  // 没咬过 → 0 档，不进 wire
                (2, [1.0, 64.0, 0.0], 2.0),  // 1 档
                (3, [2.0, 64.0, 0.0], 0.99), // 差一点到阈值 → 仍 0 档
                (4, [3.0, 64.0, 0.0], 9.0),  // 2 档
            ]
            .into_iter(),
        );
        assert_eq!(
            got,
            vec![(2, [1.0, 64.0, 0.0], 1), (4, [3.0, 64.0, 0.0], 2)],
            "只有 tier>0 的鼠进候选名单——0 档靠「缺席」表达，成群鼠患才不会撑爆 payload"
        );
    }

    #[test]
    fn tiered_candidates_all_zero_yields_empty() {
        let got = tiered_candidates(
            vec![(1, [0.0, 64.0, 0.0], 0.0), (2, [1.0, 64.0, 0.0], 0.5)].into_iter(),
        );
        assert!(
            got.is_empty(),
            "全员 0 档应得空名单（client 全量替换后所有鼠回 q0）"
        );
    }

    #[test]
    fn tiered_candidates_empty_input_yields_empty() {
        assert!(tiered_candidates(std::iter::empty()).is_empty());
    }

    // ── entries_within_radius：视距过滤 ──────────────────────────────────────

    #[test]
    fn entries_within_radius_keeps_only_visible_rats() {
        let candidates = vec![
            (1, [0.0, 64.0, 0.0], 1u8),
            (2, [100.0, 64.0, 0.0], 2u8),  // X 超出
            (3, [0.0, 64.0, 100.0], 2u8),  // Z 超出
            (4, [10.0, 64.0, -10.0], 2u8), // 边界内
        ];
        let got = entries_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0);
        assert_eq!(
            got,
            vec![
                RatQiTierEntry { id: 1, tier: 1 },
                RatQiTierEntry { id: 4, tier: 2 },
            ],
            "视距外的鼠 id 不得进 wire（与伪装名单同一条信息面收敛口径）"
        );
    }

    #[test]
    fn entries_within_radius_ignores_y_axis() {
        // chunk 可见性只按水平距离判定；头顶悬崖上的鼠不能因 Y 差被剔除。
        let candidates = vec![(1, [0.0, 300.0, 0.0], 2u8)];
        assert_eq!(
            entries_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0),
            vec![RatQiTierEntry { id: 1, tier: 2 }],
            "Y 轴不参与过滤，高差 236 格的鼠仍应在名单内"
        );
    }

    #[test]
    fn entries_within_radius_boundary_is_inclusive() {
        let candidates = vec![(1, [64.0, 64.0, 64.0], 1u8), (2, [64.01, 64.0, 0.0], 1u8)];
        assert_eq!(
            entries_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0),
            vec![RatQiTierEntry { id: 1, tier: 1 }],
            "恰好等于半径应算可见（<=），超出一丝则剔除"
        );
    }

    #[test]
    fn entries_within_radius_empty_candidates_yields_empty() {
        assert!(entries_within_radius(&[], [0.0, 64.0, 0.0], 64.0).is_empty());
    }

    /// 近处优先：输出必须按到 client 的距离升序，超限裁剪才谈得上"保近丢远"。
    #[test]
    fn entries_within_radius_sorts_nearest_first() {
        let candidates = vec![
            (1, [30.0, 64.0, 0.0], 1u8),
            (2, [5.0, 64.0, 0.0], 2u8),
            (3, [-10.0, 64.0, 0.0], 1u8),
        ];
        let ids: Vec<i32> = entries_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0)
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(
            ids,
            vec![2, 3, 1],
            "应按距离升序（5 < 10 < 30），否则超限裁剪会先丢近处的鼠"
        );
    }

    /// 距离相同时按 id 兜底排序，保证同一批输入恒得同一条 wire（不随 HashMap 迭代序漂）。
    #[test]
    fn entries_within_radius_is_stable_on_distance_ties() {
        let candidates = vec![
            (9, [3.0, 64.0, 0.0], 1u8),
            (4, [0.0, 64.0, 3.0], 1u8),
            (7, [-3.0, 64.0, 0.0], 2u8),
        ];
        let ids: Vec<i32> = entries_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0)
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, vec![4, 7, 9], "等距条目应按 id 升序，输出必须可复现");
    }

    // ── 超限裁剪：不得整包丢弃 ───────────────────────────────────────────────

    #[test]
    fn fit_keeps_everything_when_within_limit() {
        let entries: Vec<RatQiTierEntry> =
            (0..10).map(|id| RatQiTierEntry { id, tier: 2 }).collect();
        let (kept, dropped) = fit_entries_within_payload_limit(entries.clone());
        assert_eq!(dropped, 0, "未超限不应丢任何条目");
        assert_eq!(kept, entries, "未超限应原样返回");
    }

    /// 超限时必须**裁剪到装得下**并照常发送，而不是整包丢弃。
    ///
    /// 整包丢弃会让客户端档位表永不更新：已死 / 已离开视野的鼠一直亮着上一次的档位，
    /// 且下个周期同样超限，没有自愈路径。
    #[test]
    fn fit_truncates_instead_of_dropping_whole_payload() {
        let entries: Vec<RatQiTierEntry> = (0..(MAX_PAYLOAD_BYTES as i32))
            .map(|id| RatQiTierEntry { id, tier: 2 })
            .collect();
        let total = entries.len();
        let (kept, dropped) = fit_entries_within_payload_limit(entries);

        assert!(
            !kept.is_empty(),
            "超限时不得裁成空表——那等于整包丢弃，客户端 stale 档位无自愈路径"
        );
        assert_eq!(kept.len() + dropped, total, "保留数 + 丢弃数必须等于原总数");
        assert!(
            RatQiTierS2c::sync(kept.clone())
                .to_json_bytes_checked()
                .is_ok(),
            "裁剪后的 payload 必须真的能过 MAX_PAYLOAD_BYTES 校验"
        );
        // 再多留一条就应当超限——证明裁到了**最大**可装下的前缀，而不是过度保守地砍掉一大半。
        let mut one_more = kept.clone();
        one_more.push(RatQiTierEntry {
            id: i32::MAX,
            tier: 2,
        });
        assert!(
            RatQiTierS2c::sync(one_more)
                .to_json_bytes_checked()
                .is_err(),
            "应裁到恰好装得下的最大前缀（多一条即超限），否则会无谓丢掉本可下发的鼠"
        );
    }

    /// 裁剪从**末尾**（最远处）开始：近处的鼠优先保真。
    #[test]
    fn fit_drops_from_the_far_end() {
        let entries: Vec<RatQiTierEntry> = (0..(MAX_PAYLOAD_BYTES as i32))
            .map(|id| RatQiTierEntry { id, tier: 2 })
            .collect();
        let (kept, _) = fit_entries_within_payload_limit(entries.clone());
        assert_eq!(
            kept,
            entries[..kept.len()].to_vec(),
            "保留的必须是原序列的前缀（entries_within_radius 已按距离升序）——\
             从中间或开头丢会把近处的鼠丢掉"
        );
    }

    #[test]
    fn fit_on_empty_entries_is_noop() {
        let (kept, dropped) = fit_entries_within_payload_limit(Vec::new());
        assert!(kept.is_empty());
        assert_eq!(dropped, 0, "空表本就装得下，不该报丢弃");
    }
}
