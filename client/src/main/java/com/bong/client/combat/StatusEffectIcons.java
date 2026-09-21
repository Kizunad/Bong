package com.bong.client.combat;

import java.util.Set;

/** HUD 与 Inspect 共用的状态图标目录；按具体效果选择，不用类别代替效果。 */
public final class StatusEffectIcons {
    private static final String BASE = "bong-client:textures/hud/effects/";
    private static final Set<String> IDS = Set.of(
        "bleeding", "stunned", "immobilized", "vortexcasting", "parryrecovery", "staggered",
        "disoriented", "voidcoreactive", "damagereduction", "breakthroughboost",
        "antispiritpressurepill", "qiregenboost", "insightflash", "woundheal", "body_part_resist",
        "speedboost", "staminarecovboost", "health_regen_boost", "mirror_concealment",
        "swordparrying", "shieldblocking", "spirit_treasure_perception", "cultivationacceleration",
        "extraordinarymeridianacceleration", "slowed", "damageamp", "humility",
        "insighthallucination", "frailty", "qicappermminus", "contaminationboost", "body_part_weaken",
        "staminacrash", "qidrainforstamina", "legstrain", "qi_regen_paused",
        "mirror_exposed", "resonancelocked", "qiregenslowed", "damagevulnerability", "alchemy_buff", "exhausted"
    );

    private StatusEffectIcons() {}

    public static String textureFor(String id) {
        String key = id == null ? "" : id.split(":", 2)[0];
        return IDS.contains(key) ? BASE + key + ".png" : null;
    }
}
