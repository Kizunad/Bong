package com.bong.client.visual.realm_vision;

public enum SenseKind {
    LIVING_QI,
    AMBIENT_LEYLINE,
    CULTIVATOR_REALM,
    HEAVENLY_GAZE,
    CRISIS_PREMONITION,
    ZHENFA_ARRAY,
    ZHENFA_WARD_ALERT,
    SPIRIT_EYE,
    NICHE_INTRUSION_TRACE,
    /** plan-fauna-mimic-spider-v1 P2：神识识破伪装蛛，橙色 #FF8040 轮廓，Condense+ 可见。 */
    DISGUISED_SPIDER,
    /** plan-daozhan-v1 P3：神识识破伪装道伥，红色 #C04040 轮廓，Solidify(固元)+ 可见。 */
    DISGUISED_DAOZHAN,
    /** plan-dying-elder-v1 P3：感知垂死大能真元流失波动，青蓝色 #40C0E0 轮廓。 */
    DYING_ELDER_QI;

    public static SenseKind fromWire(String wireName) {
        if (wireName == null) {
            return LIVING_QI;
        }
        return switch (wireName.trim()) {
            case "AmbientLeyline" -> AMBIENT_LEYLINE;
            case "CultivatorRealm" -> CULTIVATOR_REALM;
            case "HeavenlyGaze" -> HEAVENLY_GAZE;
            case "CrisisPremonition" -> CRISIS_PREMONITION;
            case "ZhenfaArray" -> ZHENFA_ARRAY;
            case "ZhenfaWardAlert" -> ZHENFA_WARD_ALERT;
            case "SpiritEye" -> SPIRIT_EYE;
            case "NicheIntrusionTrace" -> NICHE_INTRUSION_TRACE;
            // plan-fauna-mimic-spider-v1 P2
            case "DisguisedSpider" -> DISGUISED_SPIDER;
            // plan-daozhan-v1 P3
            case "DisguisedDaoZhang" -> DISGUISED_DAOZHAN;
            // plan-dying-elder-v1 P3：垂死大能真元流失波动
            case "DyingElderQi" -> DYING_ELDER_QI;
            default -> LIVING_QI;
        };
    }
}
