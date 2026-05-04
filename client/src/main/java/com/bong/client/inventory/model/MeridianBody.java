package com.bong.client.inventory.model;

import com.bong.client.cultivation.ColorKind;

import java.util.Collections;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 人体经脉系统完整快照。
 * 包含 12 正经 + 8 奇经 + 活跃状态效果。真元池（qi_current/qi_max）由 PlayerStateHandler 独立下发，
 * 不在此结构中；世界观中真元是唯一底蕴，无三丹田分池概念。
 * 通过 {@link Builder} 构建，不可变。
 */
public final class MeridianBody {

    /** 状态效果 */
    public record StatusEffect(String id, String name, String description, int color, double severity) {}

    private final Map<MeridianChannel, ChannelState> channels;
    private final Map<MeridianChannel, InventoryItem> appliedItems;
    private final Map<MeridianChannel, Integer> cracksCount;
    private final List<StatusEffect> activeEffects;
    private final String realm;
    private final double contaminationTotal;
    private final double yearsLived;
    private final int lifespanCapByRealm;
    private final double remainingYears;
    private final int deathPenaltyYears;
    private final double lifespanTickRateMultiplier;
    private final boolean windCandle;
    private final ColorKind qiColorMain;
    private final ColorKind qiColorSecondary;
    private final boolean qiColorChaotic;
    private final boolean qiColorHunyuan;

    private MeridianBody(Builder b) {
        this.channels = Collections.unmodifiableMap(new EnumMap<>(b.channels));
        this.appliedItems = b.appliedItems.isEmpty()
            ? Map.of()
            : Collections.unmodifiableMap(new EnumMap<>(b.appliedItems));
        this.cracksCount = b.cracksCount.isEmpty()
            ? Map.of()
            : Collections.unmodifiableMap(new EnumMap<>(b.cracksCount));
        this.activeEffects = List.copyOf(b.activeEffects);
        this.realm = b.realm;
        this.contaminationTotal = Math.max(0.0, b.contaminationTotal);
        this.yearsLived = Math.max(0.0, b.yearsLived);
        this.lifespanCapByRealm = Math.max(0, b.lifespanCapByRealm);
        this.remainingYears = Math.max(0.0, b.remainingYears);
        this.deathPenaltyYears = Math.max(0, b.deathPenaltyYears);
        this.lifespanTickRateMultiplier = Math.max(0.0, b.lifespanTickRateMultiplier);
        this.windCandle = b.windCandle;
        this.qiColorMain = b.qiColorMain;
        this.qiColorSecondary = b.qiColorSecondary;
        this.qiColorChaotic = b.qiColorChaotic;
        this.qiColorHunyuan = b.qiColorHunyuan;
    }

    public ChannelState channel(MeridianChannel ch) { return channels.get(ch); }
    public Map<MeridianChannel, ChannelState> allChannels() { return channels; }
    public InventoryItem appliedItem(MeridianChannel ch) { return appliedItems.get(ch); }
    public Map<MeridianChannel, InventoryItem> allAppliedItems() { return appliedItems; }
    public List<StatusEffect> activeEffects() { return activeEffects; }
    public String realm() { return realm; }
    public double contaminationTotal() { return contaminationTotal; }
    public double yearsLived() { return yearsLived; }
    public int lifespanCapByRealm() { return lifespanCapByRealm; }
    public double remainingYears() { return remainingYears; }
    public int deathPenaltyYears() { return deathPenaltyYears; }
    public double lifespanTickRateMultiplier() { return lifespanTickRateMultiplier; }
    public boolean hasLifespanPreview() { return lifespanCapByRealm > 0; }
    public boolean isWindCandle() { return windCandle; }
    public ColorKind qiColorMain() { return qiColorMain; }
    public ColorKind qiColorSecondary() { return qiColorSecondary; }
    public boolean qiColorChaotic() { return qiColorChaotic; }
    public boolean qiColorHunyuan() { return qiColorHunyuan; }
    /** 某条经脉当前裂痕条目数；未记录则返回 0。 */
    public int cracksFor(MeridianChannel ch) {
        Integer n = cracksCount.get(ch);
        return n == null ? 0 : n;
    }

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
        ChannelState heart = channels.get(MeridianChannel.HT);
        return heart != null && (heart.damage() == ChannelState.DamageLevel.TORN
            || heart.damage() == ChannelState.DamageLevel.SEVERED);
    }

    public static Builder builder() { return new Builder(); }

    public static final class Builder {
        private final EnumMap<MeridianChannel, ChannelState> channels = new EnumMap<>(MeridianChannel.class);
        private final EnumMap<MeridianChannel, InventoryItem> appliedItems = new EnumMap<>(MeridianChannel.class);
        private final EnumMap<MeridianChannel, Integer> cracksCount = new EnumMap<>(MeridianChannel.class);
        private List<StatusEffect> activeEffects = List.of();
        private String realm = "";
        private double contaminationTotal = 0.0;
        private double yearsLived = 0.0;
        private int lifespanCapByRealm = 0;
        private double remainingYears = 0.0;
        private int deathPenaltyYears = 0;
        private double lifespanTickRateMultiplier = 0.0;
        private boolean windCandle = false;
        private ColorKind qiColorMain = ColorKind.Mellow;
        private ColorKind qiColorSecondary = null;
        private boolean qiColorChaotic = false;
        private boolean qiColorHunyuan = false;

        private Builder() {}

        public Builder channel(ChannelState state) {
            channels.put(state.channel(), state);
            return this;
        }

        public Builder channels(Map<MeridianChannel, ChannelState> all) {
            channels.putAll(all);
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

        public Builder cracksCount(Map<MeridianChannel, Integer> all) {
            cracksCount.clear();
            for (var e : all.entrySet()) {
                if (e.getValue() != null && e.getValue() > 0) {
                    cracksCount.put(e.getKey(), e.getValue());
                }
            }
            return this;
        }

        public Builder contaminationTotal(double total) {
            this.contaminationTotal = total;
            return this;
        }

        public Builder lifespanPreview(double yearsLived, int capByRealm, double remainingYears,
                                       int deathPenaltyYears, double tickRateMultiplier,
                                       boolean windCandle) {
            this.yearsLived = yearsLived;
            this.lifespanCapByRealm = capByRealm;
            this.remainingYears = remainingYears;
            this.deathPenaltyYears = deathPenaltyYears;
            this.lifespanTickRateMultiplier = tickRateMultiplier;
            this.windCandle = windCandle;
            return this;
        }

        public Builder qiColor(ColorKind main, ColorKind secondary, boolean chaotic, boolean hunyuan) {
            this.qiColorMain = main == null ? ColorKind.Mellow : main;
            this.qiColorSecondary = secondary;
            this.qiColorChaotic = chaotic;
            this.qiColorHunyuan = hunyuan;
            return this;
        }

        public MeridianBody build() {
            return new MeridianBody(this);
        }
    }
}
