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
    NICHE_INTRUSION_TRACE;

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
            default -> LIVING_QI;
        };
    }
}
