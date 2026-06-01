package com.bong.client.hud.war;

import java.util.List;

/**
 * plan-offscreen-war-v1 P9：战事 HUD 状态存储。
 *
 * <p>线程安全：volatile 快照；last-write-wins，单一 snapshot。
 * 守恒红线：本 store <b>不含任何真元字段</b>（零真元）。
 * reframe b：无具名宗门，叙事用 regionDescriptor「{zone}一带散修」。
 */
public final class FactionWarHudStore {

    /**
     * 战事 HUD 快照（plan-offscreen-war-v1 P9）。
     * <ul>
     *   <li>{@code phase} — 当前阶段（emerging/skirmish/settling/aftermath）</li>
     *   <li>{@code regionDescriptor} — 匿名区域描述符，如「残灰谷一带散修」</li>
     *   <li>{@code groups} — 参与群体 id 列表</li>
     *   <li>{@code enlistCount/mercenaryCount/interceptCount/spectateCount} — 玩家计数</li>
     *   <li>{@code winnerGroup} — 胜方 group_id（-1 表示 null）</li>
     *   <li>{@code loserGroup} — 败方 group_id（-1 表示 null）</li>
     * </ul>
     */
    public record State(
        String warId,
        String zone,
        String regionDescriptor,
        String phase,
        List<Integer> groups,
        int enlistCount,
        int mercenaryCount,
        int interceptCount,
        int spectateCount,
        int winnerGroup,
        int loserGroup
    ) {
        public static final State NONE = new State(
            "", "", "", "", List.of(), 0, 0, 0, 0, -1, -1
        );

        public State {
            zone = zone == null ? "" : zone;
            regionDescriptor = regionDescriptor == null ? "" : regionDescriptor;
            phase = phase == null ? "" : phase;
            groups = groups == null ? List.of() : List.copyOf(groups);
        }

        /** 是否有 active 战事（非空 phase，且非 aftermath/空）。 */
        public boolean active() {
            return !phase.isEmpty() && !"aftermath".equals(phase);
        }

        /** 胜方是否已确定（Settling/Aftermath 阶段）。 */
        public boolean hasOutcome() {
            return winnerGroup >= 0;
        }
    }

    private static volatile State snapshot = State.NONE;

    private FactionWarHudStore() {}

    public static State snapshot() {
        return snapshot;
    }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void clear() {
        snapshot = State.NONE;
    }

    /** 仅供测试使用。 */
    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
