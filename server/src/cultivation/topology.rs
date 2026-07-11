//! 经脉拓扑（plan §1.2 / §3.1 → plan-race-system-v1 P1b 通用化）。
//!
//! **P1b 起不再是全局单例 `standard()` Resource**——拓扑数据唯一真源是每个
//! `BodyPlan.meridian_profile.topology_edges`（`humanoid.json` 当前声明的 27 条边，
//! 与本文件退役前的硬编码子午流注矩阵 bit-for-bit 一致，见本文件末尾
//! `from_edges_matches_retired_hardcoded_matrix_bit_for_bit` 测试）。消费点
//! （`meridian_open` 打通邻接校验 / NPC 选招 `pick_next_meridian_to_open`）改为解析
//! 目标实体的 `BodyPlan` 后经 [`MeridianTopology::from_edges`] 现场构建——
//! `MeridianTopology` 结构体本身保留（邻接查询算法通用，不因数据来源变化而改变），
//! 只是构造函数从"硬编码矩阵"换成"读 `TopologyEdge` 切片"，key 从闭合枚举
//! `MeridianId` 换轨为 string [`MeridianChannelId`]。

use std::collections::HashMap;

use crate::body_plan::types::TopologyEdge;
use crate::cultivation::components::MeridianChannelId;

#[derive(Debug, Clone, Default)]
pub struct MeridianTopology {
    adjacency: HashMap<MeridianChannelId, Vec<MeridianChannelId>>,
}

impl MeridianTopology {
    /// 从 `BodyPlan.meridian_profile.topology_edges` 构建邻接表。`edges` 是无向声明
    /// （`validate_body_plan` 已保证不含未知 channel id 引用），本函数对每条边写入
    /// 双向邻接并去重排序（`Vec` 内顺序稳定，供 `neighbors` 消费点做确定性遍历，
    /// 避免 HashMap 迭代顺序抖动泄漏进测试断言）。
    pub fn from_edges(edges: &[TopologyEdge]) -> Self {
        let mut adjacency: HashMap<MeridianChannelId, Vec<MeridianChannelId>> = HashMap::new();
        for edge in edges {
            adjacency
                .entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
            adjacency
                .entry(edge.to.clone())
                .or_default()
                .push(edge.from.clone());
        }
        for v in adjacency.values_mut() {
            v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            v.dedup();
        }
        Self { adjacency }
    }

    pub fn neighbors(&self, id: impl Into<MeridianChannelId>) -> &[MeridianChannelId] {
        let id = id.into();
        self.adjacency.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn contains(&self, id: impl Into<MeridianChannelId>) -> bool {
        self.adjacency.contains_key(&id.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;

    fn humanoid_topology() -> MeridianTopology {
        let plan = crate::body_plan::registry::humanoid_plan_static();
        let profile = plan
            .meridian_profile
            .as_ref()
            .expect("humanoid.json must declare meridian_profile from P1a onward");
        MeridianTopology::from_edges(&profile.topology_edges)
    }

    #[test]
    fn all_20_meridians_present() {
        let t = humanoid_topology();
        for id in MeridianId::REGULAR {
            assert!(t.contains(id), "regular {id:?} missing");
            assert!(!t.neighbors(id).is_empty());
        }
        for id in MeridianId::EXTRAORDINARY {
            assert!(t.contains(id), "extraordinary {id:?} missing");
            assert!(!t.neighbors(id).is_empty());
        }
    }

    #[test]
    fn adjacency_is_symmetric() {
        let t = humanoid_topology();
        for id in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
        {
            for n in t.neighbors(*id) {
                assert!(
                    t.neighbors(n.clone()).contains(&id.channel_id()),
                    "asymmetric edge: {id:?} -> {n:?} but not back"
                );
            }
        }
    }

    #[test]
    fn regular_cycle_intact() {
        let t = humanoid_topology();
        // LU 应与 LI 和 LR 相邻
        let lu = t.neighbors(MeridianId::Lung);
        assert!(lu.contains(&MeridianId::LargeIntestine.channel_id()));
        assert!(lu.contains(&MeridianId::Liver.channel_id()));
    }

    #[test]
    fn empty_edges_produce_empty_topology() {
        let t = MeridianTopology::from_edges(&[]);
        assert!(!t.contains(MeridianId::Lung));
        assert!(t.neighbors(MeridianId::Lung).is_empty());
    }

    #[test]
    fn unknown_channel_id_is_absent_not_panicking() {
        let t = humanoid_topology();
        assert!(!t.contains(MeridianChannelId::new("whale_dorsal_fin")));
        assert!(t
            .neighbors(MeridianChannelId::new("whale_dorsal_fin"))
            .is_empty());
    }

    #[test]
    fn from_edges_matches_retired_hardcoded_matrix_bit_for_bit() {
        // 回归红线：退役前 `standard()` 硬编码矩阵与现在从 humanoid.json
        // topology_edges 派生的结果必须逐脉逐邻接 bit-for-bit 一致。
        let t = humanoid_topology();
        let expected: [(MeridianId, &[MeridianId]); 20] = [
            (
                MeridianId::Lung,
                &[
                    MeridianId::LargeIntestine,
                    MeridianId::Liver,
                    MeridianId::Ren,
                ],
            ),
            (
                MeridianId::LargeIntestine,
                &[MeridianId::Lung, MeridianId::Stomach],
            ),
            (
                MeridianId::Stomach,
                &[MeridianId::LargeIntestine, MeridianId::Spleen],
            ),
            (
                MeridianId::Spleen,
                &[MeridianId::Heart, MeridianId::Stomach],
            ),
            (
                MeridianId::Heart,
                &[MeridianId::SmallIntestine, MeridianId::Spleen],
            ),
            (
                MeridianId::SmallIntestine,
                &[MeridianId::Bladder, MeridianId::Heart],
            ),
            (
                MeridianId::Bladder,
                &[
                    MeridianId::Du,
                    MeridianId::Kidney,
                    MeridianId::SmallIntestine,
                    MeridianId::YangQiao,
                ],
            ),
            (
                MeridianId::Kidney,
                &[
                    MeridianId::Bladder,
                    MeridianId::Chong,
                    MeridianId::Du,
                    MeridianId::Pericardium,
                    MeridianId::Ren,
                    MeridianId::YinQiao,
                ],
            ),
            (
                MeridianId::Pericardium,
                &[
                    MeridianId::Kidney,
                    MeridianId::TripleEnergizer,
                    MeridianId::YinWei,
                ],
            ),
            (
                MeridianId::TripleEnergizer,
                &[
                    MeridianId::Gallbladder,
                    MeridianId::Pericardium,
                    MeridianId::YangWei,
                ],
            ),
            (
                MeridianId::Gallbladder,
                &[
                    MeridianId::Dai,
                    MeridianId::Liver,
                    MeridianId::TripleEnergizer,
                ],
            ),
            (
                MeridianId::Liver,
                &[MeridianId::Dai, MeridianId::Gallbladder, MeridianId::Lung],
            ),
            (
                MeridianId::Ren,
                &[MeridianId::Chong, MeridianId::Kidney, MeridianId::Lung],
            ),
            (
                MeridianId::Du,
                &[MeridianId::Bladder, MeridianId::Chong, MeridianId::Kidney],
            ),
            (
                MeridianId::Chong,
                &[MeridianId::Du, MeridianId::Kidney, MeridianId::Ren],
            ),
            (
                MeridianId::Dai,
                &[MeridianId::Gallbladder, MeridianId::Liver],
            ),
            (
                MeridianId::YinQiao,
                &[MeridianId::Kidney, MeridianId::YangQiao],
            ),
            (
                MeridianId::YangQiao,
                &[MeridianId::Bladder, MeridianId::YinQiao],
            ),
            (
                MeridianId::YinWei,
                &[MeridianId::Pericardium, MeridianId::YangWei],
            ),
            (
                MeridianId::YangWei,
                &[MeridianId::TripleEnergizer, MeridianId::YinWei],
            ),
        ];
        for (id, expected_neighbors) in expected {
            let mut actual: Vec<MeridianChannelId> = t.neighbors(id).to_vec();
            actual.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            let mut expected_ids: Vec<MeridianChannelId> =
                expected_neighbors.iter().map(|m| m.channel_id()).collect();
            expected_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            assert_eq!(
                actual, expected_ids,
                "{id:?} 的拓扑邻接必须与退役前硬编码矩阵一致"
            );
        }
    }
}
