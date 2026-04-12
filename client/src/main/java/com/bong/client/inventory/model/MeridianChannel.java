package com.bong.client.inventory.model;

/**
 * 中医经络学标准 — 12 正经 + 8 奇经 = 20 条经脉。
 *
 * <p>12 正经按手/足 × 阴/阳 × 太/少/厥(明) 三组分布于四肢躯干，呈左右对称
 * (本 UI 将手三阴归左臂、手三阳归右臂、足三阴归左腿、足三阳归右腿，
 * 仅作可视化分组，不代表中医解剖学事实)。
 *
 * <p>8 奇经不按脏腑分类，纵横全身，调节正经气血盈亏。
 */
public enum MeridianChannel {
    // ===== 12 正经 =====
    // 手三阴 (左臂)
    LU("手太阴肺经", "肺-胸-拇指，主呼吸吐纳", BodyRegion.LEFT_ARM, 0xFF88BBDD),
    HT("手少阴心经", "心-腋-小指，主神明气血", BodyRegion.LEFT_ARM, 0xFFCC4466),
    PC("手厥阴心包经", "心包-臂内-中指，护心要冲", BodyRegion.LEFT_ARM, 0xFFCC6688),
    // 手三阳 (右臂)
    LI("手阳明大肠经", "食指-臂外-面，主排浊", BodyRegion.RIGHT_ARM, 0xFFDDAA66),
    SI("手太阳小肠经", "小指-臂背-耳，主受盛化物", BodyRegion.RIGHT_ARM, 0xFFCC8855),
    TE("手少阳三焦经", "无名指-臂外-眉，主水道气化", BodyRegion.RIGHT_ARM, 0xFFCC9944),
    // 足三阴 (左腿)
    SP("足太阴脾经", "足-腿内-胁，后天之本", BodyRegion.LEFT_LEG, 0xFFCC9966),
    KI("足少阴肾经", "足心-腿内-胸，先天之本", BodyRegion.LEFT_LEG, 0xFF7799CC),
    LR("足厥阴肝经", "足-腿内-肋，藏血调气", BodyRegion.LEFT_LEG, 0xFF66AA88),
    // 足三阳 (右腿)
    ST("足阳明胃经", "面-胸腹-足，水谷之海", BodyRegion.RIGHT_LEG, 0xFFCCAA55),
    BL("足太阳膀胱经", "目-背-足，气化水液", BodyRegion.RIGHT_LEG, 0xFF6688CC),
    GB("足少阳胆经", "目-侧身-足，主决断", BodyRegion.RIGHT_LEG, 0xFFAABB66),

    // ===== 8 奇经 =====
    REN("任脉", "前正中，阴脉之海", BodyRegion.TORSO, 0xFF44AACC),
    DU("督脉", "后正中，阳脉之海", BodyRegion.TORSO, 0xFF44CCAA),
    CHONG("冲脉", "深部纵行，十二经之海", BodyRegion.TORSO, 0xFFAA88EE),
    DAI("带脉", "腰间环行，约束诸经", BodyRegion.TORSO, 0xFFEEBB44),
    YIN_WEI("阴维脉", "维系诸阴经", BodyRegion.TORSO, 0xFF8888CC),
    YANG_WEI("阳维脉", "维系诸阳经", BodyRegion.TORSO, 0xFFCC8888),
    YIN_QIAO("阴跷脉", "下肢阴侧上行至目", BodyRegion.TORSO, 0xFF88CCAA),
    YANG_QIAO("阳跷脉", "下肢阳侧上行至目", BodyRegion.TORSO, 0xFFCCAA88);

    public enum BodyRegion {
        HEAD, CHEST, TORSO, ABDOMEN, LEFT_ARM, RIGHT_ARM, LEFT_LEG, RIGHT_LEG
    }

    /** 大类：12 正经 vs 8 奇经 */
    public enum Family { REGULAR, EXTRAORDINARY }

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

    public Family family() {
        return ordinal() < 12 ? Family.REGULAR : Family.EXTRAORDINARY;
    }
}
