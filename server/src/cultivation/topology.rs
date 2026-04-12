//! 经脉拓扑（plan §1.2 / §3.1）。
//!
//! 标准 TCM 子午流注循环：
//!   LU → LI → ST → SP → HT → SI → BL → KI → PC → TE → GB → LR → LU
//!
//! 奇经八脉暂按主干接驳（Ren / Du 连接督任，Chong / Dai 连接带脉核心，
//! 四维跷脉 pair 起来），后续切片若有更精细的论据再调整。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

use super::components::MeridianId;

#[derive(Debug, Clone, Resource)]
pub struct MeridianTopology {
    adjacency: HashMap<MeridianId, Vec<MeridianId>>,
}

impl Default for MeridianTopology {
    fn default() -> Self {
        Self::standard()
    }
}

impl MeridianTopology {
    /// 标准拓扑 — P1 只提供一张，后续可扩展异体 / 特殊体质。
    pub fn standard() -> Self {
        use MeridianId::*;

        // 十二正经按子午流注首尾相接
        let regular_cycle = [
            Lung,
            LargeIntestine,
            Stomach,
            Spleen,
            Heart,
            SmallIntestine,
            Bladder,
            Kidney,
            Pericardium,
            TripleEnergizer,
            Gallbladder,
            Liver,
        ];

        let mut adjacency: HashMap<MeridianId, Vec<MeridianId>> = HashMap::new();
        for (i, id) in regular_cycle.iter().enumerate() {
            let prev = regular_cycle[(i + regular_cycle.len() - 1) % regular_cycle.len()];
            let next = regular_cycle[(i + 1) % regular_cycle.len()];
            adjacency.insert(*id, vec![prev, next]);
        }

        // 奇经接驳：
        //   Ren（任脉）— 胸腹正中，接 Lung / Kidney
        //   Du（督脉）— 背脊正中，接 Bladder / Kidney
        //   Chong（冲脉）— 接 Ren / Du / Kidney
        //   Dai（带脉）— 腰带，接 Gallbladder / Liver
        //   YinQiao / YangQiao — 下肢，接 Kidney / Bladder
        //   YinWei / YangWei — 维系，接 Pericardium / TripleEnergizer
        let extras: [(MeridianId, &[MeridianId]); 8] = [
            (Ren, &[Lung, Kidney, Chong]),
            (Du, &[Bladder, Kidney, Chong]),
            (Chong, &[Ren, Du, Kidney]),
            (Dai, &[Gallbladder, Liver]),
            (YinQiao, &[Kidney, YangQiao]),
            (YangQiao, &[Bladder, YinQiao]),
            (YinWei, &[Pericardium, YangWei]),
            (YangWei, &[TripleEnergizer, YinWei]),
        ];
        for (id, neigh) in extras {
            adjacency.insert(id, neigh.to_vec());
            // 双向补齐
            for n in neigh {
                adjacency.entry(*n).or_default().push(id);
            }
        }

        // 去重
        for v in adjacency.values_mut() {
            v.sort_by_key(|m| format!("{m:?}"));
            v.dedup();
        }

        Self { adjacency }
    }

    pub fn neighbors(&self, id: MeridianId) -> &[MeridianId] {
        self.adjacency.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn contains(&self, id: MeridianId) -> bool {
        self.adjacency.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_20_meridians_present() {
        let t = MeridianTopology::standard();
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
        let t = MeridianTopology::standard();
        for id in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
        {
            for n in t.neighbors(*id) {
                assert!(
                    t.neighbors(*n).contains(id),
                    "asymmetric edge: {id:?} -> {n:?} but not back"
                );
            }
        }
    }

    #[test]
    fn regular_cycle_intact() {
        let t = MeridianTopology::standard();
        // LU 应与 LI 和 LR 相邻
        let lu = t.neighbors(MeridianId::Lung);
        assert!(lu.contains(&MeridianId::LargeIntestine));
        assert!(lu.contains(&MeridianId::Liver));
    }
}
