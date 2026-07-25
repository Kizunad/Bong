//! SoundRecipe registry: JSON-defined vanilla sound layers for audio v1.

pub mod ambient;
pub mod implementation;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use valence::prelude::{App, Resource};

use crate::schema::audio::SoundRecipe;

pub const DEFAULT_AUDIO_RECIPES_DIR: &str = "assets/audio/recipes";

pub type RecipeId = String;

#[derive(Debug, Default)]
pub struct SoundRecipeRegistry {
    recipes: HashMap<RecipeId, SoundRecipe>,
}

impl Resource for SoundRecipeRegistry {}

#[derive(Debug)]
pub enum SoundRecipeLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Duplicate(RecipeId),
    Invalid {
        path: PathBuf,
        recipe_id: RecipeId,
        reason: String,
    },
    Empty(PathBuf),
}

impl std::fmt::Display for SoundRecipeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io: {error}"),
            Self::Json { path, source } => write!(f, "json: {}: {source}", path.display()),
            Self::Duplicate(id) => write!(f, "duplicate sound recipe id {id}"),
            Self::Invalid {
                path,
                recipe_id,
                reason,
            } => write!(
                f,
                "invalid sound recipe `{recipe_id}` at {}: {reason}",
                path.display()
            ),
            Self::Empty(path) => write!(
                f,
                "audio recipe directory {} contains no *.json files",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SoundRecipeLoadError {}

impl From<std::io::Error> for SoundRecipeLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl SoundRecipeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, recipe: SoundRecipe) -> Result<(), SoundRecipeLoadError> {
        recipe
            .validate()
            .map_err(|reason| SoundRecipeLoadError::Invalid {
                path: PathBuf::new(),
                recipe_id: recipe.id.clone(),
                reason,
            })?;
        if self.recipes.contains_key(&recipe.id) {
            return Err(SoundRecipeLoadError::Duplicate(recipe.id));
        }
        self.recipes.insert(recipe.id.clone(), recipe);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&SoundRecipe> {
        self.recipes.get(id)
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn load_default() -> Result<Self, SoundRecipeLoadError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_AUDIO_RECIPES_DIR);
        Self::load_dir(path)
    }

    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, SoundRecipeLoadError> {
        let dir = path.as_ref();
        let mut json_paths: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                    .then_some(path)
            })
            .collect();
        json_paths.sort();

        if json_paths.is_empty() {
            return Err(SoundRecipeLoadError::Empty(dir.to_path_buf()));
        }

        let mut registry = Self::new();
        for path in json_paths {
            let text = fs::read_to_string(&path)?;
            let recipe: SoundRecipe =
                serde_json::from_str(&text).map_err(|source| SoundRecipeLoadError::Json {
                    path: path.clone(),
                    source,
                })?;
            let id = recipe.id.clone();
            recipe
                .validate()
                .map_err(|reason| SoundRecipeLoadError::Invalid {
                    path: path.clone(),
                    recipe_id: id,
                    reason,
                })?;
            registry.insert(recipe).map_err(|error| match error {
                SoundRecipeLoadError::Invalid {
                    reason, recipe_id, ..
                } => SoundRecipeLoadError::Invalid {
                    path: path.clone(),
                    recipe_id,
                    reason,
                },
                other => other,
            })?;
        }
        Ok(registry)
    }
}

pub fn register(app: &mut App) {
    let registry = SoundRecipeRegistry::load_default().unwrap_or_else(|error| {
        panic!("[bong][audio] failed to load sound recipe registry: {error}");
    });
    tracing::info!(
        "[bong][audio] loaded {} sound recipe(s) from {}",
        registry.len(),
        DEFAULT_AUDIO_RECIPES_DIR
    );
    app.insert_resource(registry);
    app.init_resource::<implementation::AudioImplementationDedup>();
    ambient::register(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_audio_recipes() {
        let registry =
            SoundRecipeRegistry::load_default().expect("default audio recipes should load");
        assert_eq!(
            registry.len(),
            271,
            "audio registry should exclude removed slide and double-jump movement recipes \
             plus include 7 supply_coffin recipes (break + open common/rare/precious + emerge) \
             plus 1 ambient_dan_zong recipe \
             plus 1 ambient_wangyintai recipe \
             plus 1 offscreen_relic_reveal recipe (plan-offscreen-war-v1 P3) \
             plus 1 coffin_reclaim recipe (plan-coffin-tiers-v1 P2 对峙修复) \
             plus 1 niche_repair recipe (plan-niche-craft-fix-v1 P1) \
             plus 4 tiandao hunt ambient recipes \
             plus 3 workbench runtime recipes (place/break/open) \
             plus 1 furniture aura hint recipe \
             plus 2 trap runtime P1 recipes (beast_trap_snap/trip_wire_trigger) \
             plus 1 trap runtime P2 recipe (bait_stake_break) \
             plus 1 halfstep_rechallenge_trigger_player recipe (plan-halfstep-rechallenge-integration-v1 P0) \
             plus 2 halfstep P1 recipes (halfstep_quota_release_broadcast + halfstep_rechallenge_trigger_zone_echo) \
             plus 5 placeable container runtime recipes \
             plus 1 dead_drop_ward_break recipe \
             plus 5 woliu erosion-path recipes (woliu_ambient_vortex / woliu_void_vortex / woliu_swallowing_vortex / woliu_vortex_echo / woliu_void_core) \
             plus 1 tribulation_ascend_success recipe (AV r3-P3#3 渡劫成功 AV) \
             plus 5 sword_path cast recipes (sword_condense_edge / sword_qi_slash / sword_resonance / \
             sword_manifest_summon / sword_manifest_strike — plan-sword-path-v2 P4 server AV emit 接线) \
             plus 1 beng_quan recipe (崩拳专属施法音效，不再借用 baomai_hit_heavy 通用槽) \
             plus 6 anqi cast recipes (anqi_charge_seal / anqi_single_snipe / anqi_multi_shot / \
             anqi_soul_inject / anqi_armor_pierce / anqi_echo_fractal — 暗器六招 server AV emit 接线，\
             全部复用 vanilla 音色分层，无新音频文件) \
             plus 5 woliu 基础招式 AV 差异化 recipes (woliu_hold_sustain / woliu_burst_pop / \
             woliu_mouth_funnel / woliu_pull_drag / woliu_heart_field — 持涡/瞬涡/涡口/涡引/涡心 \
             各招专属施法音效，全部复用 vanilla 音色分层，无新音频文件) \
             plus 1 guangbo_ticao_practice recipe (广播体操练习完成 AV — 皮革整甲伸展声 + \
             紫水晶清音正反馈，全部复用 vanilla 音色分层，无新音频文件) \
             plus 5 heiwushi boss action recipes (heiwushi_melee_slash / heiwushi_dark_barrage / \
             heiwushi_dark_vortex / heiwushi_transform / heiwushi_death — plan-sword-path-complete §B \
             黑武士 boss action server 端 AV emit 接线，全部复用 vanilla 音色分层，无新音频文件) \
             plus 1 rat_bite_nip recipe (plan-ambient-threat-v1 P2 鼠患骚扰咬击 SFX，\
             entity.silverfish.ambient pitch 0.7 vol 0.5，无新音频文件) \
             plus 2 combat-hit-location-v1 P3 部位差异 recipes (combat_hit_head_crit / combat_hit_limb \
             — 头部命中叠加 attack.crit+arrow.hit_player 双层、四肢命中换成更闷的 attack.weak，\
             全部复用 vanilla 音色分层，无新音频文件) \
             plus 1 fauna_mundane_wither recipe (plan-mundane-fauna-v1 P2 负灵域灭杀消亡音效，\
             entity.wither.hurt pitch 1.6 vol 0.4，无新音频文件) \
             plus 2 sword swing recipes (sword_cleave_swing / sword_thrust_swing — 基础剑技\
             挥动破空声，空挥可闻；命中冲击音另走 CombatEvent 层。attack.nodamage 音源\
             劈低频/刺高频差异化，无新音频文件) \
             plus 1 yixing_cast recipe (plan-race-system-v1 PR-5b 易形施法音效，\
             evoker.prepare_wololo + illusioner.mirror_move + amethyst_block.chime 三层\
             变形类音色，无新音频文件) \
             plus 1 heaven_gate_charge recipe (plan-fpv-cast-av-v1 P4 天门蓄力专属配方——\
             复用 release 的 bong:skill.sword_path.heaven_gate 签名 ogg 作 pitch 0.72/vol 0.4 \
             前兆 L0 + amethyst 铺底；蓄力不再借用 sword_basics 共享的 sword_infuse，避免签名\
             泄漏到普通注剑)"
        );
        assert!(
            registry.get("fauna_mundane_wither").is_some(),
            "plan-mundane-fauna-v1 P2 负灵域灭杀 recipe `fauna_mundane_wither` 必须加载\
             （server mundane_fauna_negative_zone_wither_system 引用）"
        );
        for body_part_recipe in ["combat_hit_head_crit", "combat_hit_limb"] {
            assert!(
                registry.get(body_part_recipe).is_some(),
                "plan-combat-hit-location-v1 P3 部位差异音效 recipe `{body_part_recipe}` 必须加载\
                 （server emit_combat_audio_triggers 经 combat_hit_recipe_for_body_part 引用）"
            );
        }
        for heiwushi_recipe in [
            "heiwushi_melee_slash",
            "heiwushi_dark_barrage",
            "heiwushi_dark_vortex",
            "heiwushi_transform",
            "heiwushi_death",
        ] {
            assert!(
                registry.get(heiwushi_recipe).is_some(),
                "黑武士 boss 招式音效 recipe `{heiwushi_recipe}` 必须加载（server heiwushi_av_trigger emit 引用）"
            );
        }
        assert!(
            registry.get("guangbo_ticao_practice").is_some(),
            "广播体操练习音效 recipe `guangbo_ticao_practice` 必须加载（\
             cast_emit::tick_casts_or_interrupt 在 body.guangbo_ticao cast 完成时 emit 引用）"
        );
        for anqi_recipe in [
            "anqi_charge_seal",
            "anqi_single_snipe",
            "anqi_multi_shot",
            "anqi_soul_inject",
            "anqi_armor_pierce",
            "anqi_echo_fractal",
        ] {
            assert!(
                registry.get(anqi_recipe).is_some(),
                "暗器六招施法音效 recipe `{anqi_recipe}` 必须加载（server emit_anqi_audio_triggers 引用）"
            );
        }
        assert!(
            registry.get("beast_trap_snap").is_some(),
            "plan-trap-runtime-v1 P1 困兽夹咬合音效 recipe 必须加载"
        );
        assert!(
            registry.get("trip_wire_trigger").is_some(),
            "plan-trap-runtime-v1 P1 绊线触发音效 recipe 必须加载"
        );
        assert!(
            registry.get("bait_stake_break").is_some(),
            "plan-trap-runtime-v1 P2 诱饵桩碎裂音效 recipe 必须加载"
        );
        assert!(
            registry.get("niche_repair").is_some(),
            "plan-niche-craft-fix-v1 P1 灵龛修补音效 recipe 必须加载"
        );
        assert!(
            registry.get("tiandao_watch_ambient").is_some(),
            "plan-tiandao-hunt-v1 P1 Watch 环境音 recipe 必须加载"
        );
        assert!(
            registry.get("tiandao_pressure_ambient").is_some(),
            "plan-tiandao-hunt-v1 P1 Pressure 环境音 recipe 必须加载"
        );
        assert!(
            registry.get("tiandao_tribulation_ambient").is_some(),
            "plan-tiandao-hunt-v1 P1 Tribulation 环境音 recipe 必须加载"
        );
        assert!(
            registry.get("tiandao_annihilate_ambient").is_some(),
            "plan-tiandao-hunt-v1 P1 Annihilate 环境音 recipe 必须加载"
        );
        assert!(
            registry.get("offscreen_relic_reveal").is_some(),
            "plan-offscreen-war-v1 P3 battlefield relic reveal audio recipe must load"
        );
        assert!(registry.get("coffin_enter").is_some());
        assert!(registry.get("coffin_exit").is_some());
        assert!(registry.get("coffin_ambient").is_some());
        assert!(registry.get("coffin_break").is_some());
        assert!(
            registry.get("coffin_reclaim").is_some(),
            "plan-coffin-tiers-v1 P2 对峙修复：reclaim 专属音效 recipe 必须加载"
        );
        assert!(
            registry.get("workbench_place").is_some(),
            "plan-workbench-place-runtime-v1 P2：制作台放置音效 recipe 必须加载"
        );
        assert!(
            registry.get("workbench_break").is_some(),
            "plan-workbench-place-runtime-v1 P2：制作台拆除音效 recipe 必须加载"
        );
        assert!(
            registry.get("workbench_open").is_some(),
            "plan-workbench-place-runtime-v1 P2：制作台打开音效 recipe 必须加载"
        );
        for key in [
            "container_place",
            "container_place_deaddrop",
            "container_open",
            "container_open_deaddrop",
            "container_break",
            "dead_drop_ward_break",
        ] {
            assert!(
                registry.get(key).is_some(),
                "plan-placeable-container-blocks-v1 P2/P3：容器音效 recipe `{key}` 必须加载"
            );
        }
        // plan-supply-coffin-v1 P2.2 audio
        assert!(registry.get("supply_coffin_break_common").is_some());
        assert!(registry.get("supply_coffin_break_rare").is_some());
        assert!(registry.get("supply_coffin_break_precious").is_some());
        assert!(registry.get("supply_coffin_emerge").is_some());
        assert!(registry.get("supply_coffin_open_common").is_some());
        assert!(registry.get("supply_coffin_open_rare").is_some());
        assert!(registry.get("supply_coffin_open_precious").is_some());
        assert!(registry.get("pill_consume").is_some());
        assert!(registry.get("locust_swarm_warning").is_some());
        assert!(registry.get("tribulation_thunder_distant").is_some());
        assert!(registry.get("skill_lv_up").is_some());
        assert!(
            registry.get("furniture_aura_hint").is_some(),
            "plan-furniture-buff-v1 P3：床/蒲团 aura 上身轻提示音 recipe 必须加载"
        );
        assert!(registry.get("yidao_meridian_repair").is_some());
        assert!(registry.get("zhenmai_parry_thud").is_some());
        assert!(registry.get("zhenmai_neutralize_hiss").is_some());
        assert!(registry.get("zhenmai_shield_hum").is_some());
        assert!(registry.get("zhenmai_sever_crack").is_some());
        assert!(registry.get("vortex_low_hum").is_some());
        assert!(registry.get("vortex_qi_siphon").is_some());
        assert!(registry.get("lingtian_plant_seed").is_some());
        assert!(registry.get("lingtian_drain").is_some());
        assert!(registry.get("don_skin_low_thud").is_some());
        assert!(registry.get("shed_skin_burst").is_some());
        assert!(registry.get("contam_transfer_hum").is_some());
        assert!(registry.get("ground_crack_rumble").is_some());
        assert!(registry.get("beast_migration_rumble").is_some());
        assert!(registry.get("pillar_eruption_boom").is_some());
        assert!(registry.get("pressure_collapse_whoosh").is_some());
        assert!(registry.get("aftershock_wind").is_some());
        assert!(registry.get("tsy_race_out_alarm").is_some());
        assert!(registry.get("tsy_collapse_rumble").is_some());
        assert!(registry.get("tsy_extract_success").is_some());
        assert!(registry.get("tsy_search_scrape").is_some());
        assert!(registry.get("fauna_rat_squeal").is_some());
        assert!(
            registry.get("rat_bite_nip").is_some(),
            "plan-ambient-threat-v1 P2：鼠患骚扰咬击 SFX recipe 必须加载"
        );
        assert!(registry.get("fauna_rat_death").is_some());
        assert!(registry.get("fauna_fuya_pressure_hum").is_some());
        assert!(registry.get("fauna_fuya_charge").is_some());
        assert!(registry.get("fauna_ash_spider_attack").is_some());
        assert!(registry.get("fauna_hybrid_beast_death").is_some());
        assert!(registry.get("fauna_void_distorted_ambient").is_some());
        assert!(registry.get("dugu_needle_hiss").is_some());
        assert!(registry.get("dugu_self_cure_drink").is_some());
        assert!(registry.get("dugu_curse_cackle").is_some());
        assert!(registry.get("mountain_shake_rumble").is_some());
        assert!(registry.get("blood_burn_sizzle").is_some());
        assert!(registry.get("transcendence_thunder").is_some());
        assert!(
            registry.get("tribulation_ascend_success").is_some(),
            "AV r3-P3#3: 渡劫成功专属音效 recipe 必须加载（Ascended/HalfStep 结算时播放）"
        );
        assert!(registry.get("woliu_vacuum_palm").is_some());
        assert!(registry.get("woliu_vortex_shield").is_some());
        assert!(registry.get("woliu_vacuum_lock").is_some());
        assert!(registry.get("woliu_vortex_resonance").is_some());
        assert!(registry.get("woliu_turbulence_burst").is_some());
        // plan-woliu-path-v1 虚蚀路径 5 招式音效 recipes
        assert!(
            registry.get("woliu_ambient_vortex").is_some(),
            "AmbientVortex erosion skill sound recipe must load"
        );
        assert!(
            registry.get("woliu_void_vortex").is_some(),
            "VoidVortex erosion skill sound recipe must load"
        );
        assert!(
            registry.get("woliu_swallowing_vortex").is_some(),
            "SwallowingVortex erosion skill sound recipe must load"
        );
        assert!(
            registry.get("woliu_vortex_echo").is_some(),
            "VortexEcho erosion skill sound recipe must load"
        );
        assert!(
            registry.get("woliu_void_core").is_some(),
            "VoidCore erosion skill sound recipe must load"
        );
        // AV 差异化：woliu 基础 5 招专属施法音效 recipes（持涡/瞬涡/涡口/涡引/涡心）
        assert!(
            registry.get("woliu_hold_sustain").is_some(),
            "Hold (持涡) cast sound recipe must load"
        );
        assert!(
            registry.get("woliu_burst_pop").is_some(),
            "Burst (瞬涡) cast sound recipe must load"
        );
        assert!(
            registry.get("woliu_mouth_funnel").is_some(),
            "Mouth (涡口) cast sound recipe must load"
        );
        assert!(
            registry.get("woliu_pull_drag").is_some(),
            "Pull (涡引) cast sound recipe must load"
        );
        assert!(
            registry.get("woliu_heart_field").is_some(),
            "Heart (涡心) cast sound recipe must load"
        );
        assert!(registry.get("npc_refuse").is_some());
        assert!(registry.get("npc_greeting_cultivator").is_some());
        assert!(registry.get("npc_greeting_commoner").is_some());
        assert!(registry.get("npc_hurt").is_some());
        assert!(registry.get("npc_death").is_some());
        assert!(registry.get("npc_aggro").is_some());
        assert!(registry.get("ambient_spawn_plain").is_some());
        assert!(registry.get("ambient_tsy").is_some());
        assert!(registry.get("combat_music").is_some());
        assert!(registry.get("cultivation_meditate").is_some());
        assert!(registry.get("meridian_open_chime").is_some());
        assert!(registry.get("tribulation_atmosphere").is_some());
        assert!(registry.get("calamity_thunder").is_some());
        assert!(registry.get("calamity_miasma").is_some());
        assert!(registry.get("calamity_meridian_seal").is_some());
        assert!(registry.get("calamity_daoxiang_spawn").is_some());
        assert!(registry.get("calamity_heavenly_fire").is_some());
        assert!(registry.get("calamity_pressure_invert").is_some());
        assert!(registry.get("calamity_all_wither").is_some());
        assert!(registry.get("calamity_realm_collapse").is_some());
        assert!(registry.get("armor_break").is_some());
        assert!(registry.get("movement_dash").is_some());
        assert!(
            registry
                .get("halfstep_rechallenge_trigger_player")
                .is_some(),
            "plan-halfstep-rechallenge-integration-v1 P0：半步重渡触发音效 recipe 必须加载"
        );
        assert!(
            registry.get("halfstep_quota_release_broadcast").is_some(),
            "plan-halfstep-rechallenge-integration-v1 P1：名额释放广播音效 recipe 必须加载"
        );
        assert!(
            registry
                .get("halfstep_rechallenge_trigger_zone_echo")
                .is_some(),
            "plan-halfstep-rechallenge-integration-v1 P1：半步重渡 zone echo 音效 recipe 必须加载"
        );
        for key in [
            "sword_cleave",
            "sword_thrust",
            "sword_parry",
            "sword_infuse",
            "pill_huo_xue_consume",
            "pill_xu_gu_consume",
            "pill_duan_xu_consume",
            "pill_tie_bi_consume",
            "pill_jin_zhong_consume",
            "pill_ning_jia_consume",
            "pill_ji_feng_consume",
            "pill_suo_di_consume",
            "pill_hui_li_consume",
            "pill_hu_gu_consume",
        ] {
            assert!(
                registry.get(key).is_some(),
                "expected bundled audio recipe `{key}` to be registered"
            );
        }
        assert!(registry.get("hit_light").is_some());
        assert!(registry.get("parry_perfect").is_some());
        assert!(registry.get("breakthrough_guyuan").is_some());
        assert!(registry.get("forge_hammer_heavy").is_some());
        assert!(registry.get("alchemy_bubble").is_some());
        assert!(registry.get("lingtian_till").is_some());
        assert!(registry.get("pact_bind").is_some());
        assert!(registry.get("npc_footstep_water").is_some());
        assert!(registry.get("ambient_detail_tsy_metal_echo").is_some());
        assert!(registry.get("baomai_hit_critical").is_some());
        assert!(
            registry.get("beng_quan").is_some(),
            "崩拳专属施法音效 recipe `beng_quan` 必须加载（不再借用 baomai_hit_heavy 通用槽）"
        );
        assert!(registry.get("dugu_poison_signature").is_some());
        for key in [
            "gather_herb_tick",
            "gather_mine_tick",
            "gather_chop_tick",
            "gather_complete",
            "gather_perfect",
        ] {
            assert!(
                registry.get(key).is_some(),
                "expected audio registry to contain `{key}` because gathering-ux added a recipe JSON for that cue; actual lookup returned None"
            );
        }
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let recipe = SoundRecipeRegistry::load_default()
            .expect("default recipes should load")
            .get("pill_consume")
            .expect("fixture recipe exists")
            .clone();
        let mut registry = SoundRecipeRegistry::new();
        registry.insert(recipe.clone()).expect("first insert ok");
        assert!(matches!(
            registry.insert(recipe),
            Err(SoundRecipeLoadError::Duplicate(id)) if id == "pill_consume"
        ));
    }

    /// plan-fpv-cast-av-v1 P4 —— 跨端签名音效契约（server 侧半，client 侧半在
    /// `SignatureAudioContractTest`）：扫所有 server recipe 引用的 `bong:` sound 事件，
    /// 断言它们全部 ⊆ client `assets/bong/sounds.json` 注册的事件键。任一 recipe 指向
    /// 未注册的 `bong:` 事件 → 运行时静默无声 = 判红。
    #[test]
    fn signature_recipes_reference_registered_bong_events() {
        use std::collections::HashSet;
        use std::path::Path;

        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let recipe_dir = crate_dir.join("assets/audio/recipes");
        let sounds_json_path =
            crate_dir.join("../client/src/main/resources/assets/bong/sounds.json");

        let sounds_raw = std::fs::read_to_string(&sounds_json_path).unwrap_or_else(|error| {
            panic!(
                "读 client sounds.json 失败 {}: {error} —— P4 应已提交该文件",
                sounds_json_path.display()
            )
        });
        let sounds: serde_json::Value =
            serde_json::from_str(&sounds_raw).expect("client sounds.json 应为合法 JSON");
        let registered: HashSet<String> = sounds
            .as_object()
            .expect("sounds.json 顶层应为对象")
            .keys()
            .cloned()
            .collect();
        assert!(
            registered.len() >= 8,
            "client sounds.json 应至少注册 8 条 signature 事件，实际 {}",
            registered.len()
        );

        let mut bong_refs = 0usize;
        for entry in std::fs::read_dir(&recipe_dir).expect("server recipe 目录应存在") {
            let path = entry.expect("recipe entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("读 recipe 文件");
            let recipe: serde_json::Value = serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("recipe {} 非法 JSON: {error}", path.display()));
            let Some(layers) = recipe.get("layers").and_then(|l| l.as_array()) else {
                continue;
            };
            for layer in layers {
                let Some(sound) = layer.get("sound").and_then(|s| s.as_str()) else {
                    continue;
                };
                if let Some(event) = sound.strip_prefix("bong:") {
                    bong_refs += 1;
                    assert!(
                        registered.contains(event),
                        "recipe {} 引用 bong: 事件 `{sound}` 但 client sounds.json 未注册 `{event}` \
                         —— 运行时静默无声（server↔client 音效契约破裂）",
                        path.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        }
        assert!(
            bong_refs >= 9,
            "server recipe 应至少含 9 条 bong: signature L0 引用（9 招签名：sword_path.heaven_gate \
             release + charge 前兆均在 server 侧、woliu/zhenmai/baomai/dugu/tuike/anqi/morph 各一），\
             实际 {bong_refs}"
        );
    }

    /// **运行时消费** pin：逐项锁定「signature 招式**实际 emit** 的 recipe」的 L0 主层是其 `bong:`
    /// 事件、且 pitch 符合设计（release/常规招原速 1.0；天门蓄力前兆压调 0.72）。
    ///
    /// **关键——recipe id 一律从生产映射取，测试内不另抄一份表**：每招调用**真实映射函数**
    /// （`sword_path_recipe_for_skill` / `baomai_recipe_for_skill` / `ZhenmaiSkillId::audio_recipe`）
    /// 或引用生产 `pub(crate) const` 单一真源（`WOLIU_VOID_CORE_RECIPE` / `SHED_SKIN_BURST_RECIPE` /
    /// `DUGU_POISON_SIGNATURE_RECIPE` / `YIXING_CAST_RECIPE` / `ANQI_ECHO_FRACTAL_RECIPE`）——都是生产
    /// emit 路径真正消费的同一个值。于是：
    /// - 招式改播别的 recipe（映射函数/const 改指向）→ 本测试跟着查那个新 recipe，若它没接 `bong:`
    ///   主层就撞红；
    /// - 目标 recipe 退回 vanilla（L0 不再是 `bong:`）→ 撞红。
    ///
    /// 这正是防 P4 那类 bug 的回归门：P4 曾把签名塞进「同名但招式**不消费**」的死 recipe
    /// （heaven_gate release 塞 `heaven_gate_release` 但招式实际播 `sword_manifest_strike`；蓄力塞
    /// `heaven_gate_charge_2s` 但招式实际播专属 `heaven_gate_charge`；tuike 塞 `tuike_signature` 但招式
    /// 实际播 `shed_skin_burst`），静态「recipe 文件引用 bong:」测试假绿、实机零签名音。旧版本把 recipe
    /// id 手写死同样假绿（映射漂移检测不到），故此处务必从生产入口取 id。
    #[test]
    fn each_signature_skill_actually_emitted_recipe_swaps_l0_to_its_bong_event() {
        use crate::body_plan::morph::YIXING_CAST_RECIPE;
        use crate::combat::baomai_v3::BaomaiSkillId;
        use crate::combat::dugu_v2::skills::DUGU_POISON_SIGNATURE_RECIPE;
        use crate::combat::tuike_v2::events::SHED_SKIN_BURST_RECIPE;
        use crate::combat::woliu_v2::skills::WOLIU_VOID_CORE_RECIPE;
        use crate::combat::zhenmai_v2::ZhenmaiSkillId;
        use crate::network::audio_trigger::{
            baomai_recipe_for_skill, sword_path_recipe_for_skill, ANQI_ECHO_FRACTAL_RECIPE,
        };
        use crate::sword_path::av_event::SwordPathSkillId;
        use std::path::Path;

        let recipe_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/audio/recipes");
        // (从生产映射取到的 recipe id, 期望 L0 bong: 事件, 期望 L0 pitch, 期望 L0 最小 delay_ticks)
        // min_delay：常规招原速即响=0；**天门蓄力前兆延迟到蓄力尾段（尾程）**——招式于蓄力
        // 起始 emit，delay_ticks 把压调签名推到临界→释放前（≥60 tick）才响，兑现 plan 的
        // 「charge 尾程」而非 charge 开始就播。
        let pins: [(&str, &str, f64, u64); 9] = [
            (
                sword_path_recipe_for_skill(SwordPathSkillId::HeavenGateRelease),
                "bong:skill.sword_path.heaven_gate",
                1.0,
                0,
            ),
            (
                // 天门蓄力尾程前兆：复用 release 的签名 ogg，pitch 0.72 压调 + delay 到尾段作预示（专属 heaven_gate_charge）。
                sword_path_recipe_for_skill(SwordPathSkillId::HeavenGateCharge),
                "bong:skill.sword_path.heaven_gate",
                0.72,
                60,
            ),
            (
                baomai_recipe_for_skill(BaomaiSkillId::FullPowerRelease),
                "bong:skill.baomai.full_power_release",
                1.0,
                0,
            ),
            (
                ZhenmaiSkillId::SeverChain.audio_recipe(),
                "bong:skill.zhenmai.sever_chain",
                1.0,
                0,
            ),
            (WOLIU_VOID_CORE_RECIPE, "bong:skill.woliu.void_core", 1.0, 0),
            (SHED_SKIN_BURST_RECIPE, "bong:skill.tuike.shed", 1.0, 0),
            (
                DUGU_POISON_SIGNATURE_RECIPE,
                "bong:skill.dugu.infuse_poison",
                1.0,
                0,
            ),
            (YIXING_CAST_RECIPE, "bong:skill.morph.yixing", 1.0, 0),
            (
                ANQI_ECHO_FRACTAL_RECIPE,
                "bong:skill.anqi.echo_fractal",
                1.0,
                0,
            ),
        ];
        for (recipe_id, expected_event, expected_pitch, min_delay_ticks) in pins {
            let path = recipe_dir.join(format!("{recipe_id}.json"));
            let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "读生产映射取到的 signature recipe `{recipe_id}` 失败 {}: {error}\
                     （招式→recipe 映射指向了一个不存在的 recipe 文件）",
                    path.display()
                )
            });
            let recipe: serde_json::Value = serde_json::from_str(&raw).expect("recipe 合法 JSON");
            let layers = recipe["layers"].as_array().expect("recipe 应有 layers");
            let l0 = layers
                .first()
                .unwrap_or_else(|| panic!("signature recipe {recipe_id} 至少应有 L0 主层"));
            assert_eq!(
                l0.get("sound").and_then(|s| s.as_str()),
                Some(expected_event),
                "招式实际 emit 的 recipe `{recipe_id}`（生产映射取得）的 **L0 主层** sound 应 == \
                 {expected_event}——若签名被挪到铺底层 / recipe 退回 vanilla / 映射改指向没接签名的 \
                 recipe，实机零签名音，此断言即该回归门"
            );
            let pitch = l0
                .get("pitch")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or_else(|| panic!("signature recipe {recipe_id} 的 L0 应有 pitch"));
            assert!(
                (pitch - expected_pitch).abs() < 1e-9,
                "signature recipe {recipe_id} 的 L0 pitch 应 == {expected_pitch}\
                 （release/常规招原速 1.0；天门蓄力尾程前兆压调 0.72），实际 {pitch}"
            );
            // 音量必须 > 0——签名主层若 volume=0 则实机静音（虽 sound/pitch 对也零签名音），
            // 这是 reviewer 指出的假绿缺口：静音 recipe 仍会通过 sound/pitch 断言。
            let volume = l0
                .get("volume")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or_else(|| panic!("signature recipe {recipe_id} 的 L0 应有 volume"));
            assert!(
                volume > 0.0,
                "signature recipe {recipe_id} 的 L0 volume 必须 > 0（volume=0 = 实机静音签名，\
                 即使 sound/pitch 正确也零签名音），实际 {volume}"
            );
            // delay_ticks 下界：天门蓄力签名必须延迟到尾段（≥60 tick）才响，兑现 plan 的
            // 「charge 尾程」；常规招 min=0（原速即响）。
            let delay = l0
                .get("delay_ticks")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| panic!("signature recipe {recipe_id} 的 L0 应有 delay_ticks"));
            assert!(
                delay >= min_delay_ticks,
                "signature recipe {recipe_id} 的 L0 delay_ticks 应 >= {min_delay_ticks}\
                 （天门蓄力签名须延迟到蓄力尾段才响=charge 尾程，非蓄力起始就播），实际 {delay}"
            );
        }
    }
}
