package com.bong.client.inventory.model;

import java.util.Collections;
import java.util.EnumMap;
import java.util.Map;

/**
 * 体表状态快照。包含 16 个身体部位的伤势 + 已用物品（绷带/夹板等）。
 * 通过 {@link Builder} 构建，不可变。
 *
 * <p>核心查询接口：
 * <ul>
 *   <li>{@link #canUseHand(Side)} — 手臂链是否完整，用于限制装备槽</li>
 *   <li>{@link #legImpairment(Side)} — 腿部功能性，用于计算移速（TODO: 服务端）</li>
 *   <li>{@link #isBleeding()} — 是否有任何部位在出血</li>
 * </ul>
 */
public final class PhysicalBody {

    public enum Side { LEFT, RIGHT }

    /** 移速影响等级 */
    public enum MovementImpairment {
        NONE("正常", 1.0),
        LIMP("跛行", 0.7),
        CRIPPLED("严重跛行", 0.4),
        IMMOBILE("无法行走", 0.0);

        private final String label;
        private final double speedMultiplier;

        MovementImpairment(String label, double speedMultiplier) {
            this.label = label;
            this.speedMultiplier = speedMultiplier;
        }

        public String label() { return label; }
        public double speedMultiplier() { return speedMultiplier; }
    }

    private final Map<BodyPart, BodyPartState> parts;
    private final Map<BodyPart, InventoryItem> appliedItems;

    private PhysicalBody(Builder b) {
        EnumMap<BodyPart, BodyPartState> p = new EnumMap<>(BodyPart.class);
        // 所有未指定的部位默认完好
        for (BodyPart bp : BodyPart.values()) {
            p.put(bp, b.parts.getOrDefault(bp, BodyPartState.intact(bp)));
        }
        this.parts = Collections.unmodifiableMap(p);
        this.appliedItems = b.appliedItems.isEmpty()
            ? Map.of()
            : Collections.unmodifiableMap(new EnumMap<>(b.appliedItems));
    }

    public BodyPartState part(BodyPart bp) { return parts.get(bp); }
    public Map<BodyPart, BodyPartState> allParts() { return parts; }
    public InventoryItem appliedItem(BodyPart bp) { return appliedItems.get(bp); }
    public Map<BodyPart, InventoryItem> allAppliedItems() { return appliedItems; }

    /**
     * 手臂是否可用（整条手臂链路必须没有断肢）。
     * 用于限制 MAIN_HAND(右)/OFF_HAND(左) 装备槽。
     */
    public boolean canUseHand(Side side) {
        if (side == Side.LEFT) {
            return !part(BodyPart.LEFT_UPPER_ARM).wound().isSevered()
                && !part(BodyPart.LEFT_FOREARM).wound().isSevered()
                && !part(BodyPart.LEFT_HAND).wound().isSevered();
        } else {
            return !part(BodyPart.RIGHT_UPPER_ARM).wound().isSevered()
                && !part(BodyPart.RIGHT_FOREARM).wound().isSevered()
                && !part(BodyPart.RIGHT_HAND).wound().isSevered();
        }
    }

    /**
     * 腿部伤势对移速的影响。
     * TODO: 服务端根据此值应用 Slowness 效果。
     */
    public MovementImpairment legImpairment(Side side) {
        BodyPartState thigh, calf, foot;
        if (side == Side.LEFT) {
            thigh = part(BodyPart.LEFT_THIGH);
            calf = part(BodyPart.LEFT_CALF);
            foot = part(BodyPart.LEFT_FOOT);
        } else {
            thigh = part(BodyPart.RIGHT_THIGH);
            calf = part(BodyPart.RIGHT_CALF);
            foot = part(BodyPart.RIGHT_FOOT);
        }

        // 大腿断 → 无法行走
        if (thigh.wound().isSevered()) return MovementImpairment.IMMOBILE;
        // 小腿断 → 严重跛行
        if (calf.wound().isSevered()) return MovementImpairment.CRIPPLED;
        // 脚断 → 跛行
        if (foot.wound().isSevered()) return MovementImpairment.LIMP;
        // 骨折
        if (thigh.wound() == WoundLevel.FRACTURE) return MovementImpairment.CRIPPLED;
        if (calf.wound() == WoundLevel.FRACTURE) return MovementImpairment.LIMP;
        if (foot.wound() == WoundLevel.FRACTURE) return MovementImpairment.LIMP;

        return MovementImpairment.NONE;
    }

    /** 综合双腿取最差的那条 */
    public MovementImpairment worstLegImpairment() {
        MovementImpairment left = legImpairment(Side.LEFT);
        MovementImpairment right = legImpairment(Side.RIGHT);
        return left.ordinal() > right.ordinal() ? left : right;
    }

    public boolean isBleeding() {
        return parts.values().stream().anyMatch(s -> s.bleedRate() > 0.01);
    }

    public boolean hasAnyWound() {
        return parts.values().stream().anyMatch(s -> s.wound() != WoundLevel.INTACT);
    }

    public static Builder builder() { return new Builder(); }

    public static final class Builder {
        private final EnumMap<BodyPart, BodyPartState> parts = new EnumMap<>(BodyPart.class);
        private final EnumMap<BodyPart, InventoryItem> appliedItems = new EnumMap<>(BodyPart.class);

        private Builder() {}

        public Builder part(BodyPartState state) {
            parts.put(state.part(), state);
            return this;
        }

        public Builder wound(BodyPart part, WoundLevel wound) {
            parts.put(part, new BodyPartState(part, wound, 0, 0, false));
            return this;
        }

        public Builder wound(BodyPart part, WoundLevel wound, double bleedRate) {
            parts.put(part, new BodyPartState(part, wound, bleedRate, 0, false));
            return this;
        }

        public Builder wound(BodyPart part, WoundLevel wound, double bleedRate, double healProgress, boolean splinted) {
            parts.put(part, new BodyPartState(part, wound, bleedRate, healProgress, splinted));
            return this;
        }

        public Builder appliedItem(BodyPart part, InventoryItem item) {
            appliedItems.put(part, item);
            return this;
        }

        public PhysicalBody build() {
            return new PhysicalBody(this);
        }
    }
}
