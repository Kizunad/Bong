pub const COMMAND_NAMES: &[&str] = &[
    // plan-dandao-runtime-wiring-v1 P4：暴龙王 BOSS dev spawn 命令
    "baolongwang",
    "bong",
    "clearinv",
    // plan-offscreen-war-v1 P6：玩家参与涌现冲突（/faction join|mercenary|intercept）
    "faction",
    "give",
    "gm",
    "health",
    "kill",
    "meridian",
    "npc_scenario",
    "ping",
    "preview_tp",
    "qi",
    "rat",
    "realm",
    "reset",
    "revive",
    "season",
    "shader_push",
    "shrine",
    "spawn",
    "stamina",
    "summon",
    "supply_coffin",
    "technique",
    "time",
    "top",
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
    // plan-dandao-runtime-wiring-v1 P4：暴龙王 BOSS dev spawn 命令
    "baolongwang spawn",
    "bong breakthrough",
    "bong combat <target:string> <qi_invest:double>",
    "bong gather <resource:string>",
    "clearinv",
    "clearinv <scope:string>",
    // plan-offscreen-war-v1 P6：/faction 玩家参与涌现冲突
    "faction intercept",
    "faction join <group:integer>",
    "faction mercenary <group:integer>",
    "give <id:string>",
    "give <id:string> <count:integer>",
    "gm <mode:string>",
    "health set <value:float>",
    "kill self",
    "meridian list",
    "meridian open <id:string>",
    "meridian open_all",
    "npc_scenario <scenario:string>",
    "ping",
    "preview_tp <x:double> <y:double> <z:double> <yaw:float> <pitch:float>",
    "qi max <value:double>",
    "qi set <value:double>",
    "rat activate",
    "realm set <id:string>",
    "reset",
    "revive self",
    "season advance <amount:string>",
    "season query",
    "season set <phase:string>",
    "shader_push broadcast",
    "shader_push set <name:string> <value:double>",
    "shrine <action:string>",
    "spawn",
    "stamina set <value:float>",
    "summon heiwushi",
    "summon rat",
    "supply_coffin cooldown <grade:string> <secs:integer>",
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
    "top",
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
                "baolongwang",
                "bong",
                "clearinv",
                "faction",
                "give",
                "gm",
                "health",
                "kill",
                "meridian",
                "npc_scenario",
                "ping",
                "preview_tp",
                "qi",
                "rat",
                "realm",
                "reset",
                "revive",
                "season",
                "shader_push",
                "shrine",
                "spawn",
                "stamina",
                "summon",
                "supply_coffin",
                "technique",
                "time",
                "top",
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
