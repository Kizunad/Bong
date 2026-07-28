package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 并发修复（@hive blocker）：
 * 按维度独立存储 echo / charge / abrasion / aim，各自持有独立 expiresAtMillis 和 lastTick。
 *
 * <p>旧 replace() 语义下：server 分三路发 echo/charge/abrasion，后到的 replace 清零其他字段，
 * 三路无法并存。新语义：每路只更新自己维度，不触碰其他维度。
 *
 * <p>{@link #snapshot(long)} 把所有未过期维度合并成单个 {@link AnqiHudState}，
 * {@link AnqiHudPlanner#buildCommands} 逐字段渲染逻辑不变。
 *
 * <p>tick 守序（fix 2）：每维度记录 lastTick，仅 tick >= lastTick 的 payload 才 update，
 * 静默丢弃乱序旧包（tick < lastTick）。同帧不同维度允许相同 tick。
 */
public final class AnqiHudStateStore {

    /** 单维度快照（不可变）。每个字段只对应该维度有意义的数据。 */
    private record DimSlot(
        float value1,       // aim: aimProgress / echo: unused / charge: chargeProgress / abrasion: abrasionQiPayload
        int   intValue,     // echo: echoCount / others: 0
        String strValue,    // abrasion: container / others: ""
        long  expiresAtMillis,
        long  lastTick
    ) {
        static DimSlot empty() {
            return new DimSlot(0f, 0, "", 0L, Long.MIN_VALUE);
        }

        static DimSlot aim(float progress, long expiresAt, long tick) {
            return new DimSlot(AnqiHudState.clamp01(progress), 0, "", expiresAt, tick);
        }

        static DimSlot echo(int count, long expiresAt, long tick) {
            return new DimSlot(0f, Math.max(0, count), "", expiresAt, tick);
        }

        static DimSlot charge(float progress, long expiresAt, long tick) {
            return new DimSlot(AnqiHudState.clamp01(progress), 0, "", expiresAt, tick);
        }

        static DimSlot abrasion(String container, float qiPayload, long expiresAt, long tick) {
            String c = container == null ? "" : container;
            return new DimSlot(Math.max(0f, qiPayload), 0, c, expiresAt, tick);
        }

        static DimSlot multiShot(int count, long expiresAt, long tick) {
            return new DimSlot(0f, Math.max(0, count), "", expiresAt, tick);
        }

        boolean active(long nowMillis) {
            return expiresAtMillis > nowMillis;
        }
    }

    // 每个维度独立 AtomicReference，互不干扰
    private static final AtomicReference<DimSlot> AIM_SLOT       = new AtomicReference<>(DimSlot.empty());
    private static final AtomicReference<DimSlot> ECHO_SLOT      = new AtomicReference<>(DimSlot.empty());
    private static final AtomicReference<DimSlot> CHARGE_SLOT    = new AtomicReference<>(DimSlot.empty());
    private static final AtomicReference<DimSlot> ABRASION_SLOT  = new AtomicReference<>(DimSlot.empty());
    private static final AtomicReference<DimSlot> MULTISHOT_SLOT = new AtomicReference<>(DimSlot.empty());

    private AnqiHudStateStore() {}

    // ─── 维度更新 API（handler 调用）────────────────────────────────

    /**
     * 更新 aim 维度。tick < slot.lastTick 的包静默丢弃（乱序保护）。
     */
    public static void updateAim(float progress, long nowMillis, long durationMillis, long tick) {
        long expiresAt = nowMillis + Math.max(1L, durationMillis);
        updateSlotCas(AIM_SLOT, tick, DimSlot.aim(progress, expiresAt, tick));
    }

    /**
     * 更新 echo 维度。tick < slot.lastTick 的包静默丢弃（乱序保护）。
     */
    public static void updateEcho(int count, long nowMillis, long durationMillis, long tick) {
        long expiresAt = nowMillis + Math.max(1L, durationMillis);
        updateSlotCas(ECHO_SLOT, tick, DimSlot.echo(count, expiresAt, tick));
    }

    /**
     * 更新 charge 维度。tick < slot.lastTick 的包静默丢弃（乱序保护）。
     */
    public static void updateCharge(float progress, long nowMillis, long durationMillis, long tick) {
        long expiresAt = nowMillis + Math.max(1L, durationMillis);
        updateSlotCas(CHARGE_SLOT, tick, DimSlot.charge(progress, expiresAt, tick));
    }

    /**
     * 更新 abrasion 维度。tick < slot.lastTick 的包静默丢弃（乱序保护）。
     */
    public static void updateAbrasion(String container, float qiPayload, long nowMillis, long durationMillis, long tick) {
        long expiresAt = nowMillis + Math.max(1L, durationMillis);
        updateSlotCas(ABRASION_SLOT, tick, DimSlot.abrasion(container, qiPayload, expiresAt, tick));
    }

    /**
     * 更新 multishot 维度（多发齐射弹数）。tick < slot.lastTick 的包静默丢弃（乱序保护）。
     */
    public static void updateMultiShot(int count, long nowMillis, long durationMillis, long tick) {
        long expiresAt = nowMillis + Math.max(1L, durationMillis);
        updateSlotCas(MULTISHOT_SLOT, tick, DimSlot.multiShot(count, expiresAt, tick));
    }

    // ─── 合并快照（Planner 调用）─────────────────────────────────────

    /**
     * 把所有未过期维度合并成单个 {@link AnqiHudState}。
     * 已过期维度贡献零值（等同从结果中剔除）。
     * Planner 的逐字段渲染逻辑无需修改。
     */
    public static AnqiHudState snapshot(long nowMillis) {
        DimSlot aim       = AIM_SLOT.get();
        DimSlot echo      = ECHO_SLOT.get();
        DimSlot charge    = CHARGE_SLOT.get();
        DimSlot abrasion  = ABRASION_SLOT.get();
        DimSlot multiShot = MULTISHOT_SLOT.get();

        float  aimProgress    = aim.active(nowMillis)       ? aim.value1()        : 0f;
        float  chargeProgress = charge.active(nowMillis)    ? charge.value1()     : 0f;
        int    echoCount      = echo.active(nowMillis)      ? echo.intValue()     : 0;
        String abrasionCont   = abrasion.active(nowMillis)  ? abrasion.strValue() : "";
        float  abrasionQi     = abrasion.active(nowMillis)  ? abrasion.value1()   : 0f;
        int    multiShotCount = multiShot.active(nowMillis) ? multiShot.intValue(): 0;

        // expiresAtMillis: 取所有维度中最大值（保证 AnqiHudState.active() 判断正确）
        long expiresAt = Math.max(
            Math.max(
                Math.max(aim.expiresAtMillis(), echo.expiresAtMillis()),
                Math.max(charge.expiresAtMillis(), abrasion.expiresAtMillis())
            ),
            multiShot.expiresAtMillis()
        );

        return new AnqiHudState(aimProgress, chargeProgress, echoCount, abrasionCont, abrasionQi, multiShotCount, expiresAt);
    }

    /**
     * 无参 snapshot（向后兼容旧调用点，使用 currentTimeMillis）。
     */
    public static AnqiHudState snapshot() {
        return snapshot(System.currentTimeMillis());
    }

    // ─── 清除 ─────────────────────────────────────────────────────

    public static void clear() {
        AIM_SLOT.set(DimSlot.empty());
        ECHO_SLOT.set(DimSlot.empty());
        CHARGE_SLOT.set(DimSlot.empty());
        ABRASION_SLOT.set(DimSlot.empty());
        MULTISHOT_SLOT.set(DimSlot.empty());
    }

    // ─── 遗留兼容：replace(state)（仅测试向后兼容，生产已改 updateXxx） ─

    /**
     * @deprecated 使用 {@link #updateEcho}/{@link #updateCharge}/{@link #updateAbrasion} 代替。
     *             保留仅为 legacy 测试向后兼容；生产 handler 已改用 updateXxx()。
     */
    @Deprecated
    public static void replace(AnqiHudState state) {
        if (state == null) {
            clear();
            return;
        }
        long expiry = state.expiresAtMillis();
        long tick   = Long.MAX_VALUE; // replace 不参与 tick 守序，使用最大 tick 确保写入
        AIM_SLOT.set(DimSlot.aim(state.aimProgress(), expiry, tick));
        ECHO_SLOT.set(DimSlot.echo(state.echoCount(), expiry, tick));
        CHARGE_SLOT.set(DimSlot.charge(state.chargeProgress(), expiry, tick));
        ABRASION_SLOT.set(DimSlot.abrasion(
            state.abrasionContainer() == null ? "" : state.abrasionContainer(),
            state.abrasionQiPayload(), expiry, tick));
        MULTISHOT_SLOT.set(DimSlot.multiShot(state.multiShotCount(), expiry, tick));
    }

    // ─── 内部 CAS 循环 ────────────────────────────────────────────

    /**
     * CAS 循环：仅当 newTick >= current.lastTick 时才 compareAndSet（乱序包丢弃）。
     */
    private static void updateSlotCas(AtomicReference<DimSlot> ref, long newTick, DimSlot newSlot) {
        DimSlot current;
        do {
            current = ref.get();
            if (newTick < current.lastTick()) {
                return; // 乱序旧包，静默丢弃
            }
        } while (!ref.compareAndSet(current, newSlot));
    }
}
