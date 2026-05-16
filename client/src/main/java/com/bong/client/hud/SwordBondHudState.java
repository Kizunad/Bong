package com.bong.client.hud;

public final class SwordBondHudState {
    private final boolean active;
    private final int grade;
    private final String gradeName;
    private final float storedQiRatio;
    private final float bondStrength;
    private final boolean heavenGateReady;

    public static final SwordBondHudState INACTIVE = new SwordBondHudState(
        false, 0, "", 0f, 0f, false
    );

    public SwordBondHudState(
        boolean active,
        int grade,
        String gradeName,
        float storedQiRatio,
        float bondStrength,
        boolean heavenGateReady
    ) {
        this.active = active;
        this.grade = grade;
        this.gradeName = gradeName != null ? gradeName : "";
        this.storedQiRatio = clamp01(storedQiRatio);
        this.bondStrength = clamp01(bondStrength);
        this.heavenGateReady = heavenGateReady;
    }

    public boolean active() { return active; }
    public int grade() { return grade; }
    public String gradeName() { return gradeName; }
    public float storedQiRatio() { return storedQiRatio; }
    public float bondStrength() { return bondStrength; }
    public boolean heavenGateReady() { return heavenGateReady; }

    static float clamp01(float v) {
        if (Float.isNaN(v)) return 0f;
        return Math.max(0f, Math.min(1f, v));
    }
}
