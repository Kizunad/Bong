package com.bong.client.inventory.model;

import java.util.Collections;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 人体经脉系统完整快照。
 * 包含 12 条经脉状态 + 3 个丹田 + 活跃状态效果。
 * 通过 {@link Builder} 构建，不可变。
 */
public final class MeridianBody {

    /** 丹田等级 */
    public enum DantianTier {
        UPPER("上丹田", "神识之海"),
        MIDDLE("中丹田", "气海"),
        LOWER("下丹田", "精元之源");

        private final String name;
        private final String desc;

        DantianTier(String name, String desc) { this.name = name; this.desc = desc; }

        public String displayName() { return name; }
        public String description() { return desc; }
    }

    /** 丹田状态 */
    public record DantianState(DantianTier tier, double current, double max, boolean sealed) {
        public double ratio() { return max > 0 ? Math.min(1, current / max) : 0; }
    }

    /** 状态效果 */
    public record StatusEffect(String id, String name, String description, int color, double severity) {}

    private final Map<MeridianChannel, ChannelState> channels;
    private final Map<DantianTier, DantianState> dantians;
    private final Map<MeridianChannel, InventoryItem> appliedItems;
    private final List<StatusEffect> activeEffects;
    private final String realm;

    private MeridianBody(Builder b) {
        this.channels = Collections.unmodifiableMap(new EnumMap<>(b.channels));
        this.dantians = Collections.unmodifiableMap(new EnumMap<>(b.dantians));
        this.appliedItems = b.appliedItems.isEmpty()
            ? Map.of()
            : Collections.unmodifiableMap(new EnumMap<>(b.appliedItems));
        this.activeEffects = List.copyOf(b.activeEffects);
        this.realm = b.realm;
    }

    public ChannelState channel(MeridianChannel ch) { return channels.get(ch); }
    public Map<MeridianChannel, ChannelState> allChannels() { return channels; }
    public DantianState dantian(DantianTier tier) { return dantians.get(tier); }
    public Map<DantianTier, DantianState> allDantians() { return dantians; }
    public InventoryItem appliedItem(MeridianChannel ch) { return appliedItems.get(ch); }
    public Map<MeridianChannel, InventoryItem> allAppliedItems() { return appliedItems; }
    public List<StatusEffect> activeEffects() { return activeEffects; }
    public String realm() { return realm; }

    /** 全身平均有效流量比 */
    public double overallFlowHealth() {
        double sum = 0;
        int count = 0;
        for (ChannelState cs : channels.values()) {
            sum += cs.effectiveFlow() / Math.max(1, cs.capacity());
            count++;
        }
        return count > 0 ? sum / count : 0;
    }

    /** 是否有任何经脉处于危险状态 */
    public boolean hasAnyDamage() {
        return channels.values().stream()
            .anyMatch(cs -> cs.damage() != ChannelState.DamageLevel.INTACT);
    }

    /** 心脉损伤是否达到走火入魔阈值 */
    public boolean isQiDeviation() {
        ChannelState heart = channels.get(MeridianChannel.HEART);
        return heart != null && (heart.damage() == ChannelState.DamageLevel.TORN
            || heart.damage() == ChannelState.DamageLevel.SEVERED);
    }

    public static Builder builder() { return new Builder(); }

    public static final class Builder {
        private final EnumMap<MeridianChannel, ChannelState> channels = new EnumMap<>(MeridianChannel.class);
        private final EnumMap<DantianTier, DantianState> dantians = new EnumMap<>(DantianTier.class);
        private final EnumMap<MeridianChannel, InventoryItem> appliedItems = new EnumMap<>(MeridianChannel.class);
        private List<StatusEffect> activeEffects = List.of();
        private String realm = "";

        private Builder() {}

        public Builder channel(ChannelState state) {
            channels.put(state.channel(), state);
            return this;
        }

        public Builder channels(Map<MeridianChannel, ChannelState> all) {
            channels.putAll(all);
            return this;
        }

        public Builder dantian(DantianState state) {
            dantians.put(state.tier(), state);
            return this;
        }

        public Builder dantians(Map<DantianTier, DantianState> all) {
            dantians.putAll(all);
            return this;
        }

        public Builder appliedItem(MeridianChannel channel, InventoryItem item) {
            appliedItems.put(channel, item);
            return this;
        }

        public Builder appliedItems(Map<MeridianChannel, InventoryItem> all) {
            appliedItems.putAll(all);
            return this;
        }

        public Builder activeEffects(List<StatusEffect> effects) {
            this.activeEffects = effects;
            return this;
        }

        public Builder realm(String realm) {
            this.realm = realm;
            return this;
        }

        public MeridianBody build() {
            return new MeridianBody(this);
        }
    }
}
