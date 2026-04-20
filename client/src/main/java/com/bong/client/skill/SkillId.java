package com.bong.client.skill;

/**
 * plan-skill-v1 §1 首批 MVP skill 三种。serde 字符串与 server
 * {@code server::skill::components::SkillId} 对齐（snake_case）。
 */
public enum SkillId {
    HERBALISM("herbalism", "采药"),
    ALCHEMY("alchemy", "炼丹"),
    FORGING("forging", "锻造");

    private final String wire;
    private final String displayName;

    SkillId(String wire, String displayName) {
        this.wire = wire;
        this.displayName = displayName;
    }

    public String wireId() {
        return wire;
    }

    public String displayName() {
        return displayName;
    }

    /**
     * 从 wire 字符串解析；未知值返回 {@code null}（plan §3.2 "不识此技，暂不能悟"）。
     */
    public static SkillId fromWire(String wire) {
        if (wire == null) return null;
        for (SkillId id : values()) {
            if (id.wire.equals(wire)) return id;
        }
        return null;
    }
}
