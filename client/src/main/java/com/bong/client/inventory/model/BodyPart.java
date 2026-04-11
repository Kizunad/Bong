package com.bong.client.inventory.model;

/**
 * 16 个身体部位，与人体剪影渲染区域 1:1 对应。
 */
public enum BodyPart {
    HEAD("头部", BodyGroup.HEAD),
    NECK("颈部", BodyGroup.HEAD),
    CHEST("胸腔", BodyGroup.TORSO),
    ABDOMEN("腹部", BodyGroup.TORSO),
    LEFT_UPPER_ARM("左上臂", BodyGroup.LEFT_ARM),
    LEFT_FOREARM("左前臂", BodyGroup.LEFT_ARM),
    LEFT_HAND("左手", BodyGroup.LEFT_ARM),
    RIGHT_UPPER_ARM("右上臂", BodyGroup.RIGHT_ARM),
    RIGHT_FOREARM("右前臂", BodyGroup.RIGHT_ARM),
    RIGHT_HAND("右手", BodyGroup.RIGHT_ARM),
    LEFT_THIGH("左大腿", BodyGroup.LEFT_LEG),
    LEFT_CALF("左小腿", BodyGroup.LEFT_LEG),
    LEFT_FOOT("左脚", BodyGroup.LEFT_LEG),
    RIGHT_THIGH("右大腿", BodyGroup.RIGHT_LEG),
    RIGHT_CALF("右小腿", BodyGroup.RIGHT_LEG),
    RIGHT_FOOT("右脚", BodyGroup.RIGHT_LEG);

    /** 肢体分组，用于判断整条手臂/腿的连通性 */
    public enum BodyGroup {
        HEAD, TORSO, LEFT_ARM, RIGHT_ARM, LEFT_LEG, RIGHT_LEG
    }

    private final String displayName;
    private final BodyGroup group;

    BodyPart(String displayName, BodyGroup group) {
        this.displayName = displayName;
        this.group = group;
    }

    public String displayName() { return displayName; }
    public BodyGroup group() { return group; }
}
