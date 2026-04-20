package com.bong.client.skill;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * plan-skill-v1 §8 SkillSet 客户端镜像（POJO）。对应 server {@code SkillSet} 的子集：
 * <ul>
 *   <li>{@code skills} — 按 {@link SkillId} 映射到当前 {@link Entry}</li>
 * </ul>
 *
 * <p>客户端不 mirror 服务器的 {@code consumed_scrolls}（P4 才接入残卷拖入学习）。
 *
 * <p>所有实例不可变（copy-on-write 风格）—— 用 {@link #withSkill(SkillId, Entry)} 派生新实例。
 */
public final class SkillSetSnapshot {
    /** plan §6 单条 skill 的 UI 呈现数据。 */
    public record Entry(
        int lv,
        long xp,
        long xpToNext,
        long totalXp,
        int cap,
        /** 最近一次 +XP 的数值快照（用于 plan §5.1 左列"最近 +XP"，由事件流累加/衰减）。 */
        long recentGainXp,
        /** 最近一次 +XP 的客户端时间戳（毫秒，用于 3s 窗口衰减）。 */
        long recentGainMillis
    ) {
        public Entry {
            lv = Math.max(0, Math.min(10, lv));
            xp = Math.max(0L, xp);
            xpToNext = Math.max(1L, xpToNext);
            totalXp = Math.max(0L, totalXp);
            cap = Math.max(0, Math.min(10, cap));
            recentGainXp = Math.max(0L, recentGainXp);
            recentGainMillis = Math.max(0L, recentGainMillis);
        }

        /** plan §4 effective_lv = min(real_lv, cap)。 */
        public int effectiveLv() {
            return Math.min(lv, cap);
        }

        /** plan §5.1 XP 进度条比例，0..1。 */
        public double progressRatio() {
            if (xpToNext <= 0L) return 0.0;
            return Math.max(0.0, Math.min(1.0, (double) xp / (double) xpToNext));
        }

        public static Entry zero() {
            return new Entry(0, 0L, 100L, 0L, 10, 0L, 0L);
        }
    }

    private static final SkillSetSnapshot EMPTY = new SkillSetSnapshot(Collections.emptyMap());

    private final Map<SkillId, Entry> skills;

    private SkillSetSnapshot(Map<SkillId, Entry> skills) {
        this.skills = skills;
    }

    public static SkillSetSnapshot empty() {
        return EMPTY;
    }

    public static SkillSetSnapshot of(Map<SkillId, Entry> skills) {
        if (skills == null || skills.isEmpty()) return EMPTY;
        return new SkillSetSnapshot(Collections.unmodifiableMap(new LinkedHashMap<>(skills)));
    }

    public Map<SkillId, Entry> skills() {
        return skills;
    }

    /**
     * 取指定 skill；若不存在返回 {@link Entry#zero()} —— 避免 UI 写空判，保持三行固定展示。
     */
    public Entry get(SkillId id) {
        Objects.requireNonNull(id, "id");
        Entry e = skills.get(id);
        return e != null ? e : Entry.zero();
    }

    /** 派生一个替换了指定 skill 的新 snapshot（其他 skill 保留）。 */
    public SkillSetSnapshot withSkill(SkillId id, Entry entry) {
        Objects.requireNonNull(id, "id");
        Objects.requireNonNull(entry, "entry");
        Map<SkillId, Entry> next = new LinkedHashMap<>(skills);
        next.put(id, entry);
        return new SkillSetSnapshot(Collections.unmodifiableMap(next));
    }
}
