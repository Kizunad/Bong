pub const COMMAND_NAMES: &[&str] = &[
    "bong",
    "gm",
    "health",
    "npc_scenario",
    "ping",
    "preview_tp",
    "rat",
    "season",
    "shrine",
    "spawn",
    "stamina",
    "summon",
    "top",
    "tptree",
    "tpzone",
    "tsy_spawn",
    "wound",
    "zones",
];

#[cfg(test)]
pub const COMMAND_TREE_PATHS: &[&str] = &[
    "bong breakthrough",
    "bong combat <target:string> <qi_invest:double>",
    "bong gather <resource:string>",
    "gm <mode:string>",
    "health set <value:float>",
    "npc_scenario <scenario:string>",
    "ping",
    "preview_tp <x:double> <y:double> <z:double> <yaw:float> <pitch:float>",
    "rat activate",
    "season advance <amount:string>",
    "season query",
    "season set <phase:string>",
    "shrine <action:string>",
    "spawn",
    "stamina set <value:float>",
    "summon rat",
    "summon whale",
    "top",
    "tptree <tree:string>",
    "tpzone <zone:string>",
    "tsy_spawn <family_id:string>",
    "wound add <part:string>",
    "wound add <part:string> <severity:float>",
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
                "bong",
                "gm",
                "health",
                "npc_scenario",
                "ping",
                "preview_tp",
                "rat",
                "season",
                "shrine",
                "spawn",
                "stamina",
                "summon",
                "top",
                "tptree",
                "tpzone",
                "tsy_spawn",
                "wound",
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
