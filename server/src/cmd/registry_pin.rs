pub const COMMAND_NAMES: &[&str] = &[
    "bong",
    "gm",
    "health",
    "npc_scenario",
    "ping",
    "preview_tp",
    "shrine",
    "spawn",
    "stamina",
    "top",
    "tptree",
    "tpzone",
    "tsy_spawn",
    "wound",
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
                "shrine",
                "spawn",
                "stamina",
                "top",
                "tptree",
                "tpzone",
                "tsy_spawn",
                "wound",
                "zones",
            ]
        );
    }
}
