pub const COMMAND_NAMES: &[&str] = &[
    "ambient_spawn",
    // plan-tribulation-balance-v1 P0：渡劫平衡监控看板 dev 命令
    "balance",
    // plan-dandao-runtime-wiring-v1 P4：暴龙王 BOSS dev spawn 命令
    "baolongwang",
    "bong",
    // plan-bot-e2e-coverage-v1 P4：确定性真实 Plant 采集夹具
    "botany_spawn",
    "clearinv",
    // plan-coffin-tiers-v1 P4：延寿棺档级直写 dev 命令
    "coffin",
    // plan-offscreen-war-v1 P6：玩家参与涌现冲突（/faction join|mercenary|intercept）
    "faction",
    // plan-dense-fog-v1 P1 前置：动态雾堤 dev 命令
    "fog",
    // plan-worldgen-v4 P5：画廊审阅闭环 stamp 命令（/gallery）
    "gallery",
    "give",
    "gm",
    "health",
    "identity",
    "kill",
    "meridian",
    "npc_scenario",
    "ping",
    "preview_tp",
    "qi",
    // plan-race-system-v1 P5：种族切换 dev 命令（走 RaceChange 两阶段事务）
    "race",
    "rat",
    "realm",
    "reset",
    "revive",
    // plan-sou-da-che-v1 P0：区域风险热图 GM 调试命令
    "riskmap",
    "season",
    "shader_push",
    "shrine",
    // bot e2e sparring 场景的 dev 通道（/sparring invite <username>）
    "sparring",
    "spawn",
    "stamina",
    // plan-worldgen-v4 P5：画廊审阅闭环 save 命令（/structure save <name>）
    "structure",
    "summon",
    "supply_coffin",
    "technique",
    "time",
    "top",
    "tpdim",
    "tppoi",
    "tptree",
    "tpzone",
    "tribulation_debug",
    "tribulation_rechallenge",
    "tsy_spawn",
    "whale",
    "wound",
    "zone_qi",
    "zones",
];

#[cfg(test)]
pub const COMMAND_TREE_PATHS: &[&str] = &[
    "ambient_spawn once mundane <x:double> <z:double>",
    "ambient_spawn once threat <x:double> <z:double>",
    // plan-tribulation-balance-v1 P0：渡劫平衡监控看板
    "balance tribulation",
    // plan-dandao-runtime-wiring-v1 P4：暴龙王 BOSS dev spawn 命令
    "baolongwang spawn",
    "bong breakthrough",
    "bong combat <target:string> <qi_invest:double>",
    "bong gather <resource:string>",
    // plan-bot-e2e-coverage-v1 P4：确定性真实 Plant 采集夹具
    "botany_spawn spirit_grass",
    "clearinv",
    "clearinv <scope:string>",
    // plan-coffin-tiers-v1 P4：延寿棺档级直写 dev 命令
    "coffin grade <grade:string>",
    // plan-offscreen-war-v1 P6：/faction 玩家参与涌现冲突
    // plan-faction-expansion-v1 P1：/faction list 显示三势力关系矩阵
    "faction intercept",
    "faction join <target:string>",
    "faction list",
    "faction mercenary <group:integer>",
    // plan-dense-fog-v1 P1 前置：动态雾堤 dev 命令
    "fog clear <id:string>",
    "fog clear_all",
    "fog list",
    "fog spawn <radius:double> <density:double>",
    "fog spawn <radius:double> <density:double> <duration_ticks:integer>",
    // plan-worldgen-v4 P5：画廊审阅闭环 stamp 命令（无参数）
    "gallery",
    // plan-worldgen-v4 P6：装饰虚空审阅台（出生点上空铺全部 54 个 NBT 装饰变体）
    "gallery decorations",
    "give <id:string>",
    "give <id:string> <count:integer>",
    "gm <mode:string>",
    "health set <value:float>",
    "identity list",
    "identity new <display_name:string>",
    "identity rename <new_name:string>",
    "identity switch <id:integer>",
    "kill self",
    "meridian list",
    "meridian open <id:string>",
    "meridian open_all",
    "npc_scenario <scenario:string>",
    "ping",
    "preview_tp <x:double> <y:double> <z:double> <yaw:float> <pitch:float>",
    "qi max <value:double>",
    "qi set <value:double>",
    "race set <id:string>",
    "rat activate",
    "realm set <id:string>",
    "reset",
    "revive self",
    // plan-sou-da-che-v1 P0：区域风险热图 GM 调试命令（无参数，直接输出所在 zone 风险信息）
    "riskmap",
    "season advance <amount:string>",
    "season query",
    "season set <phase:string>",
    "shader_push broadcast",
    "shader_push set <name:string> <value:double>",
    "shrine <action:string>",
    "sparring invite <target:string>",
    "spawn",
    "stamina set <value:float>",
    // plan-worldgen-v4 P5：画廊审阅闭环 save 命令（覆盖原 .nbt）
    "structure save <name:string>",
    "summon heiwushi",
    "summon rat",
    "supply_coffin barrier",
    "supply_coffin cooldown <grade:string> <secs:integer>",
    "supply_coffin lifecycle pause",
    "supply_coffin lifecycle resume",
    "supply_coffin list",
    "supply_coffin reset",
    "supply_coffin spawn <grade:string>",
    "supply_coffin tp",
    "technique active <id:string> <value:bool>",
    "technique add <id:string>",
    "technique give <id:string>",
    "technique list",
    "technique proficiency <id:string> <value:double>",
    "technique remove <id:string>",
    "technique reset_all",
    "time advance <ticks:integer>",
    "time now",
    "top",
    "tpdim <dimension:string>",
    "tppoi <zone:string> <poi:string>",
    "tppoi novice",
    "tptree <tree:string>",
    "tpzone <zone:string>",
    "tribulation_debug",
    "tribulation_rechallenge",
    "tsy_spawn <family_id:string>",
    "whale spawn",
    "wound add <part:string>",
    "wound add <part:string> <severity:float>",
    "zone_qi set <name:string> <value:double>",
    "zones",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_are_sorted_and_unique() {
        assert!(
            COMMAND_NAMES.windows(2).all(|pair| pair[0] < pair[1]),
            "COMMAND_NAMES must stay sorted so command tree diffs are reviewable"
        );
    }

    #[test]
    fn command_names_pin_flat_dev_roots_and_bong_gameplay_root() {
        assert_eq!(
            COMMAND_NAMES,
            &[
                "ambient_spawn",
                "balance",
                "baolongwang",
                "bong",
                "botany_spawn",
                "clearinv",
                "coffin",
                "faction",
                "fog",
                "gallery",
                "give",
                "gm",
                "health",
                "identity",
                "kill",
                "meridian",
                "npc_scenario",
                "ping",
                "preview_tp",
                "qi",
                "race",
                "rat",
                "realm",
                "reset",
                "revive",
                "riskmap",
                "season",
                "shader_push",
                "shrine",
                "sparring",
                "spawn",
                "stamina",
                "structure",
                "summon",
                "supply_coffin",
                "technique",
                "time",
                "top",
                "tpdim",
                "tppoi",
                "tptree",
                "tpzone",
                "tribulation_debug",
                "tribulation_rechallenge",
                "tsy_spawn",
                "whale",
                "wound",
                "zone_qi",
                "zones",
            ]
        );
    }

    #[test]
    fn command_tree_paths_are_sorted_and_unique() {
        assert!(
            COMMAND_TREE_PATHS.windows(2).all(|pair| pair[0] < pair[1]),
            "COMMAND_TREE_PATHS must stay sorted so command tree diffs are reviewable"
        );
    }
}
