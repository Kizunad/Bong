package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

/**
 * 震脉（zhenmai）5 招 + 盾格挡的客户端瞬态 HUD 状态投影。
 *
 * <p>设计沿用 {@link AnqiHudStateStore} 的「逐维度独立 slot + 各自 expiry」模型：
 * server 分多路推送（parry 成功 / neutralize / multipoint / harden / sever-chain /
 * shield-block），各路只更新自己的维度，互不覆盖归零。Planner 读合并快照按维度条件渲染。
 *
 * <p><b>条件/瞬态硬约束</b>：每个维度都带 {@code expiresAtMillis}，过期即从快照中剔除
 * （贡献「未激活」语义）。Planner 只在维度激活时 emit 命令，未激活时完全隐藏，绝不常驻。
 *
 * <p>维度划分：
 * <ul>
 *   <li>{@code PARRY} — 极限弹反成功：瞬态中心环高亮（短闪，~450ms）</li>
 *   <li>{@code NEUTRALIZE} — 局部中和：瞬态去毒提示（含 contam 去除量，~1.4s）</li>
 *   <li>{@code MULTIPOINT} — 多点反震：激活期微指示（剩余反震点数 + 持续时间，duration-based）</li>
 *   <li>{@code HARDEN} — 护脉：buff 激活期微条（剩余时长 + 减伤%，duration-based）</li>
 *   <li>{@code SEVER} — 绝脉断链：断脉标记 + 反噬增幅倒计时（amplification 窗口期内显示）</li>
 *   <li>{@code SHIELD} — 盾格挡：命中瞬态确认弧光（~350ms）</li>
 * </ul>
 *
 * <p>守恒红线：本 store 只投影 HUD 状态，绝不触碰真元/经脉/战斗 store。
 */
public final class ZhenmaiHudStateStore {

    /**
     * 单维度瞬态/持续快照（不可变）。字段语义随维度而异：
     * <ul>
     *   <li>parry/shield: 仅 expiry 有意义（纯瞬态闪现）</li>
     *   <li>neutralize: {@code value1}=contamRemoved，{@code label}=经脉名</li>
     *   <li>multipoint: {@code intValue}=剩余反震点数</li>
     *   <li>harden: {@code value1}=减伤比例 [0,1]，{@code label}=经脉名</li>
     *   <li>sever: {@code label}=断掉的经脉名，{@code value1}=k_drain（增幅强度，仅展示用）</li>
     * </ul>
     */
    public record Slot(
        float value1,
        int intValue,
        String label,
        long expiresAtMillis
    ) {
        public Slot {
            label = label == null ? "" : label;
        }

        public static final Slot EMPTY = new Slot(0f, 0, "", 0L);

        public boolean active(long nowMillis) {
            return expiresAtMillis > nowMillis;
        }
    }

    private static final AtomicReference<Slot> PARRY_SLOT = new AtomicReference<>(Slot.EMPTY);
    private static final AtomicReference<Slot> NEUTRALIZE_SLOT = new AtomicReference<>(Slot.EMPTY);
    private static final AtomicReference<Slot> MULTIPOINT_SLOT = new AtomicReference<>(Slot.EMPTY);
    private static final AtomicReference<Slot> HARDEN_SLOT = new AtomicReference<>(Slot.EMPTY);
    private static final AtomicReference<Slot> SEVER_SLOT = new AtomicReference<>(Slot.EMPTY);
    private static final AtomicReference<Slot> SHIELD_SLOT = new AtomicReference<>(Slot.EMPTY);

    private ZhenmaiHudStateStore() {
    }

    // ─── 维度更新 API（handler 调用）─────────────────────────────────

    /** 极限弹反成功：纯瞬态闪现。 */
    public static void flashParry(long nowMillis, long durationMillis) {
        PARRY_SLOT.set(new Slot(0f, 0, "", nowMillis + Math.max(1L, durationMillis)));
    }

    /** 局部中和：展示去除的染毒量 + 经脉名。 */
    public static void flashNeutralize(float contamRemoved, String meridian, long nowMillis, long durationMillis) {
        NEUTRALIZE_SLOT.set(new Slot(Math.max(0f, contamRemoved), 0, meridian, nowMillis + Math.max(1L, durationMillis)));
    }

    /** 多点反震：激活期内剩余反震点数（持续到 expiry）。 */
    public static void setMultipoint(int remainingPoints, long nowMillis, long durationMillis) {
        MULTIPOINT_SLOT.set(new Slot(0f, Math.max(0, remainingPoints), "", nowMillis + Math.max(1L, durationMillis)));
    }

    /** 护脉 buff：减伤比例 [0,1] + 经脉名（持续到 buff expiry）。 */
    public static void setHarden(float damageReduction, String meridian, long nowMillis, long durationMillis) {
        float clamped = Float.isFinite(damageReduction) ? Math.max(0f, Math.min(1f, damageReduction)) : 0f;
        HARDEN_SLOT.set(new Slot(clamped, 0, meridian, nowMillis + Math.max(1L, durationMillis)));
    }

    /** 绝脉断链：断掉的经脉 + 增幅窗口（反噬增幅持续期内显示断脉标记）。 */
    public static void setSever(String meridian, float kDrain, long nowMillis, long durationMillis) {
        SEVER_SLOT.set(new Slot(Math.max(0f, kDrain), 0, meridian, nowMillis + Math.max(1L, durationMillis)));
    }

    /** 盾格挡命中：纯瞬态确认弧光。 */
    public static void flashShieldBlock(long nowMillis, long durationMillis) {
        SHIELD_SLOT.set(new Slot(0f, 0, "", nowMillis + Math.max(1L, durationMillis)));
    }

    // ─── 合并快照（Planner 调用）─────────────────────────────────────

    public static Snapshot snapshot(long nowMillis) {
        return new Snapshot(
            PARRY_SLOT.get(),
            NEUTRALIZE_SLOT.get(),
            MULTIPOINT_SLOT.get(),
            HARDEN_SLOT.get(),
            SEVER_SLOT.get(),
            SHIELD_SLOT.get()
        );
    }

    public static Snapshot snapshot() {
        return snapshot(System.currentTimeMillis());
    }

    /** 全维度合并快照（不可变）。Planner 按维度 {@link Slot#active(long)} 决定显示。 */
    public record Snapshot(
        Slot parry,
        Slot neutralize,
        Slot multipoint,
        Slot harden,
        Slot sever,
        Slot shield
    ) {
        public static final Snapshot NONE = new Snapshot(
            Slot.EMPTY, Slot.EMPTY, Slot.EMPTY, Slot.EMPTY, Slot.EMPTY, Slot.EMPTY
        );
    }

    public static void clear() {
        PARRY_SLOT.set(Slot.EMPTY);
        NEUTRALIZE_SLOT.set(Slot.EMPTY);
        MULTIPOINT_SLOT.set(Slot.EMPTY);
        HARDEN_SLOT.set(Slot.EMPTY);
        SEVER_SLOT.set(Slot.EMPTY);
        SHIELD_SLOT.set(Slot.EMPTY);
    }

    public static void resetForTests() {
        clear();
    }
}
