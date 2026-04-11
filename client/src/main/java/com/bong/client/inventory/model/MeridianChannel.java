package com.bong.client.inventory.model;

/**
 * 12 条主要经脉，基于末法残土世界观。
 * 包含奇经二脉（任/督）、核心脉（心/神识）、四肢脉（手足阴阳）、脏腑脉（肺/肾/肝/脾）。
 */
public enum MeridianChannel {
    // 奇经
    REN_MAI("任脉", "前正中，阴脉之海", BodyRegion.TORSO, 0xFF44AACC),
    DU_MAI("督脉", "后正中，阳脉之海", BodyRegion.TORSO, 0xFF44CCAA),

    // 核心
    HEART("心脉", "气血枢纽，走火入魔触发器", BodyRegion.CHEST, 0xFFCC4466),
    SPIRIT("神识脉", "神识感知，意念操控", BodyRegion.HEAD, 0xFFAAAAFF),

    // 四肢
    ARM_YIN("手三阴", "左臂，阵法/辅助", BodyRegion.LEFT_ARM, 0xFF66AADD),
    ARM_YANG("手三阳", "右臂，器修/暗器", BodyRegion.RIGHT_ARM, 0xFFDDAA66),
    LEG_YIN("足三阴", "左足，根基/站桩", BodyRegion.LEFT_LEG, 0xFF66BBAA),
    LEG_YANG("足三阳", "右足，身法/步法", BodyRegion.RIGHT_LEG, 0xFFBBAA66),

    // 脏腑
    LUNG("肺脉", "呼吸吐纳，真元摄入", BodyRegion.CHEST, 0xFF88BBDD),
    KIDNEY("肾脉", "先天之本，境界根基", BodyRegion.ABDOMEN, 0xFF7799CC),
    LIVER("肝脉", "藏血调气，真元储备", BodyRegion.ABDOMEN, 0xFF66AA88),
    SPLEEN("脾脉", "后天之本，丹药吸收", BodyRegion.ABDOMEN, 0xFFCC9966);

    public enum BodyRegion {
        HEAD, CHEST, TORSO, ABDOMEN, LEFT_ARM, RIGHT_ARM, LEFT_LEG, RIGHT_LEG
    }

    private final String displayName;
    private final String description;
    private final BodyRegion region;
    private final int baseColor;

    MeridianChannel(String displayName, String description, BodyRegion region, int baseColor) {
        this.displayName = displayName;
        this.description = description;
        this.region = region;
        this.baseColor = baseColor;
    }

    public String displayName() { return displayName; }
    public String description() { return description; }
    public BodyRegion region() { return region; }
    public int baseColor() { return baseColor; }
}
