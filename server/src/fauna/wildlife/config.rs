//! 三种野生生物的出生属性与战术技能槽。技能成本仍由 TechniqueRegistry 唯一持有。

use serde::Deserialize;
use valence::prelude::{bevy_ecs, Component, Resource};

use crate::body_plan::{RaceId, RaceRegistry};
use crate::cultivation::components::Realm;
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::cultivation::skill_registry::SkillRegistry;
use crate::fauna::components::BeastKind;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WildlifeDefinition {
    pub kind: BeastKind,
    pub realms: Vec<RealmAttributes>,
    pub mass: f64,
    pub speed: f64,
    pub retreat_health: f32,
    pub engage_skill: String,
    pub followup_skill: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealmAttributes {
    pub realm: Realm,
    pub health: f32,
    pub attack: f32,
    /// 承伤倍率，1 为无减免。
    pub damage_taken: f32,
    pub stamina: f32,
}

#[derive(Clone, Debug, Deserialize, Resource)]
#[serde(deny_unknown_fields)]
pub struct WildlifeCatalog {
    pub creatures: Vec<WildlifeDefinition>,
}

impl WildlifeCatalog {
    pub fn load() -> Self {
        let path = crate::body_plan::resolve_assets_root().join("assets/fauna/wildlife.toml");
        let source = std::fs::read_to_string(&path).expect("读取野生生物配置");
        let catalog: Self = toml::from_str(&source).expect("解析野生生物配置");
        for kind in [
            BeastKind::DainuLion,
            BeastKind::FuyuVulture,
            BeastKind::Horse,
        ] {
            assert_eq!(
                catalog
                    .creatures
                    .iter()
                    .filter(|def| def.kind == kind)
                    .count(),
                1
            );
            let def = catalog.get(kind);
            assert!(def.mass.is_finite() && def.mass > 0.0);
            assert!(def.speed.is_finite() && def.speed > 0.0 && def.speed <= 3.0);
            assert!((0.0..1.0).contains(&def.retreat_health));
            assert_eq!(def.realms.len(), 2);
            for (index, stats) in def.realms.iter().enumerate() {
                assert_eq!(stats.realm.rank(), kind.realm_tier() + index as u8 + 1);
                for value in [
                    stats.health,
                    stats.attack,
                    stats.damage_taken,
                    stats.stamina,
                ] {
                    assert!(value.is_finite() && value > 0.0);
                }
                assert!(stats.damage_taken <= 1.0);
            }
        }
        catalog
    }

    pub fn get(&self, kind: BeastKind) -> &WildlifeDefinition {
        self.creatures
            .iter()
            .find(|def| def.kind == kind)
            .expect("已校验的野生生物配置")
    }

    /// 配置错误在启动时带物种、槽位和技能名报出，避免刷出永远不能施法的生物。
    pub fn validate_skills(
        &self,
        techniques: &TechniqueRegistry,
        skills: &SkillRegistry,
        races: &RaceRegistry,
    ) -> Result<(), String> {
        for creature in &self.creatures {
            let race = RaceId::new(creature.kind.as_str());
            for (slot, id) in [
                ("engage_skill", &creature.engage_skill),
                ("followup_skill", &creature.followup_skill),
            ] {
                let context = format!("{}.{slot} = {id}", creature.kind.as_str());
                let definition = techniques
                    .get(id)
                    .ok_or_else(|| format!("{context}: 未注册技能元数据"))?;
                if skills.lookup(id).is_none() {
                    return Err(format!("{context}: 未注册技能执行器"));
                }
                if !definition.required_race.allows(&race, false) {
                    return Err(format!("{context}: 物种不满足技能种族门"));
                }
                if !creature
                    .realms
                    .iter()
                    .any(|stats| stats.realm.rank() >= definition.required_realm_value().rank())
                {
                    return Err(format!("{context}: 出生境界均不满足技能境界门"));
                }
                for dependency in &definition.required_meridians {
                    if !races.has_channel(&race, &dependency.channel.as_str().into()) {
                        return Err(format!(
                            "{context}: 物种构型缺少经脉 {}",
                            dependency.channel
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// 长期基础属性参与每帧聚合，不能在出生时直接写 DerivedAttrs 后被下一帧清掉。
#[derive(Clone, Debug, Component)]
pub struct WildlifeAttributes {
    pub attack: f32,
    pub damage_taken: f32,
    pub stamina: f32,
}
