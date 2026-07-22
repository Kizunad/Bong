package com.bong.client.combat.juice;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.DeathStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;

import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.LongSupplier;

/**
 * plan-fpv-cast-av-v1 P3 —— 施法瞬间 juice：FOV 加法脉冲控制器 + cast 状态转换驱动的调度。
 *
 * <p><b>生命周期契约</b>（§P3）：状态机 {@code idle → pulse → decay → idle}，所有路径终点回到
 * 单一基准 FOV（加法偏移归零），复位幂等。驱动路径唯一 = {@link CastStateStore} 的状态转换回调
 * （由 {@code CastSyncHandler} / 本地预测 replace 触发）；client tick 推进 decay 与死亡 teardown。
 *
 * <p><b>门控</b>（§8.1 #3）：release juice 只绑**已 accepted（观测到 CASTING）的 cast identity**。
 * identity = {@code (source, slot, startedAtMs)} 三元组（{@link CastState} 无唯一 cast id）。
 * <ul>
 *   <li>CASTING：按 identity 武装 pending（该招在 {@link CastJuiceProfiles} 有 profile 才武装）；
 *       同 identity 重复 CASTING 幂等，异 identity 取代旧 pending（supersession）。</li>
 *   <li>COMPLETE：identity 匹配且未触发过 → 触发 FOV 脉冲 + shake，标记已触发（同 identity 重复
 *       release 幂等）。</li>
 *   <li>INTERRUPT：作废 pending，并记录被作废 identity（防**乱序**——打断早于 CASTING 到达时，
 *       后到的同 identity CASTING 不再武装）。</li>
 *   <li>IDLE：清 pending（施放前 NONE 拒绝 / 300ms 自回 idle）。</li>
 * </ul>
 * 施放前拒绝 client 从不收到 CASTING → 无 pending 可作废（对门控无害）。施法中非受击死亡 server
 * 静默移除 Casting 不发 cast_sync（§8.1 #3 唯一缺口）→ 由 {@link #tick()} 观测
 * {@link DeathStateStore} 死亡即 {@link #teardown()}，不依赖 server 回执。
 *
 * <p><b>FOV 合成</b>：只产出加法偏移量（{@link #fovDelta()}），由 {@code MixinGameRenderer} 叠加到
 * vanilla getFov 返回值上——与原版疾跑/药水/水下、shader 等 FOV 源共存，不直写绝对 FOV。
 */
public final class CastFovController {
    private static final AtomicBoolean BOOTSTRAPPED = new AtomicBoolean(false);
    /** pending / voidedId 的互斥锁（cast 转换回调与 teardown 可能跨线程）。 */
    private static final Object LOCK = new Object();

    /** 施法 release 抖动方向（自施法无攻击方向，取对角 → 偏航+俯仰混合抖动）。 */
    private static final double CAST_SHAKE_DIR_X = 1.0;
    private static final double CAST_SHAKE_DIR_Z = 1.0;

    /** 时钟 seam：生产 = {@code System.currentTimeMillis}；单测注入可推进时钟。 */
    private static volatile LongSupplier clock = System::currentTimeMillis;

    /** 全局 juice 强度倍率（0 = 关闭），默认 1.0。fovDelta 每帧乘它 → 进行中调 0 立即复位。 */
    private static volatile float multiplier = 1.0f;

    /** 当前 FOV 脉冲（不可变；volatile 供渲染线程读）。null = idle（基准 FOV）。 */
    private static volatile Pulse pulse = null;

    /** 当前武装的 pending juice（LOCK 保护）。 */
    private static Pending pending = null;
    /** 最近被作废的 cast identity（LOCK 保护；防乱序打断被后到 CASTING 复活）。 */
    private static Identity voidedId = null;

    private CastFovController() {
    }

    /** 客户端启动挂钩：cast 转换监听 + tick decay + 断线/切世界 teardown。 */
    public static void bootstrap() {
        if (!BOOTSTRAPPED.compareAndSet(false, true)) {
            return;
        }
        CastStateStore.addListener(CastFovController::onCastState);
        ClientTickEvents.END_CLIENT_TICK.register(client -> tick());
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> teardown());
    }

    /**
     * cast 状态转换回调（{@code CastSyncHandler.replace} / 本地预测 → {@code CastStateStore
     * .setSnapshot} → 本方法）。这是驱动 juice 状态机的唯一入口（不直接暴露 arm/fire 给外部）。
     */
    static void onCastState(CastState state) {
        if (state == null) {
            return;
        }
        long now = clock.getAsLong();
        synchronized (LOCK) {
            switch (state.phase()) {
                case CASTING -> arm(state);
                case COMPLETE -> release(state, now);
                case INTERRUPT -> voidPending(state);
                case IDLE -> pending = null;
            }
        }
    }

    private static void arm(CastState state) {
        Identity id = Identity.of(state);
        if (id.equals(voidedId)) {
            return;  // 乱序：该 cast 的打断早于 CASTING 到达 → 已作废，不武装
        }
        if (pending != null && !pending.id.equals(id)) {
            pending = null;  // supersession：新 cast 身份取代旧 pending
        }
        if (pending != null) {
            return;  // 同身份重复 CASTING：幂等
        }
        CastJuiceProfile profile = resolveProfile(state);
        if (profile != null) {
            pending = new Pending(id, profile);
        }
    }

    private static void release(CastState state, long now) {
        if (pending == null || !pending.id.equals(Identity.of(state)) || pending.fired) {
            return;  // 无 pending / 身份不符 / 已触发过（幂等）
        }
        pending.fired = true;
        fire(pending.profile, now);
    }

    private static void voidPending(CastState state) {
        voidedId = Identity.of(state);  // 记录被作废身份（取消令牌语义 + 防乱序复活）
        pending = null;
    }

    /** 触发 FOV 脉冲 + shake（同帧调度）。倍率在触发时刻并入 shake 强度（0 → 不触发）。 */
    private static void fire(CastJuiceProfile profile, long now) {
        if (profile.hasFovPulse()) {
            pulse = new Pulse(profile.fovPeakDegrees(), profile.fovDurationTicks(), now);
        }
        if (profile.hasShake()) {
            CameraShakeController.triggerDirect(
                profile.shakeIntensity() * multiplier,
                profile.shakeDurationTicks(),
                CAST_SHAKE_DIR_X,
                CAST_SHAKE_DIR_Z,
                false,
                now
            );
        }
    }

    /**
     * 渲染线程每帧调（{@code MixinGameRenderer}）：当前加法 FOV 偏移（0 = 基准）。倍率每帧并入 →
     * 进行中把倍率调 0 立即复位到基准。
     */
    public static double fovDelta() {
        Pulse p = pulse;
        if (p == null) {
            return 0.0;
        }
        double raw = p.offsetAt(clock.getAsLong());
        return raw == 0.0 ? 0.0 : raw * multiplier;
    }

    /** client tick：死亡 teardown（§8.1 #3 死亡缺口）+ 过期脉冲清理回 idle。 */
    public static void tick() {
        if (DeathStateStore.snapshot().visible()) {
            teardown();
            return;
        }
        Pulse p = pulse;
        if (p != null && !p.activeAt(clock.getAsLong())) {
            pulse = null;  // 脉冲自然结束 → 回 idle
        }
    }

    /** 断线 / 切世界 / 死亡：立即复位基准 FOV + 清 pending（幂等，重复调用无副作用）。 */
    public static void teardown() {
        pulse = null;
        synchronized (LOCK) {
            pending = null;
            voidedId = null;
        }
    }

    /** 全局 juice 强度倍率（0 = 关闭）。进行中调 0：fovDelta 每帧乘它已即时复位。 */
    public static void setJuiceMultiplier(float value) {
        multiplier = Float.isNaN(value) || value < 0f ? 0f : value;
    }

    public static float juiceMultiplier() {
        return multiplier;
    }

    private static CastJuiceProfile resolveProfile(CastState state) {
        return CastJuiceProfiles.get(resolveSkillId(state));
    }

    /**
     * slot → skillId。juice profile 皆技能栏技能；本地预测 {@code beginSkillBarCast}
     * （{@code SkillBarKeyRouter}）保住 SKILL_BAR source。QUICK_SLOT = 快捷物品，无技能 profile。
     */
    private static String resolveSkillId(CastState state) {
        if (state.source() != CastState.Source.SKILL_BAR) {
            return null;
        }
        SkillBarEntry entry = SkillBarStore.snapshot().slot(state.slot());
        if (entry == null || entry.kind() != SkillBarEntry.Kind.SKILL) {
            return null;
        }
        return entry.id();
    }

    // --- 测试 seam ---
    static void setClockForTest(LongSupplier c) {
        clock = c == null ? System::currentTimeMillis : c;
    }

    static void resetForTests() {
        pulse = null;
        synchronized (LOCK) {
            pending = null;
            voidedId = null;
        }
        multiplier = 1.0f;
        clock = System::currentTimeMillis;
    }

    /** FOV 脉冲：峰值 + 时长；{@code sin} 半弧「收缩回弹」（progress 0→peak→0，终点归基准）。 */
    private record Pulse(float peakDegrees, int durationTicks, long startedAtMs) {
        private long durationMs() {
            return Math.max(0, durationTicks) * 50L;
        }

        boolean activeAt(long now) {
            long elapsed = now - startedAtMs;
            return durationMs() > 0L && elapsed >= 0L && elapsed < durationMs();
        }

        double offsetAt(long now) {
            long dur = durationMs();
            long elapsed = now - startedAtMs;
            if (dur <= 0L || elapsed < 0L || elapsed >= dur) {
                return 0.0;
            }
            double progress = elapsed / (double) dur;
            return peakDegrees * Math.sin(Math.PI * progress);
        }
    }

    private record Identity(CastState.Source source, int slot, long startedAtMs) {
        static Identity of(CastState s) {
            return new Identity(s.source(), s.slot(), s.startedAtMs());
        }
    }

    private static final class Pending {
        final Identity id;
        final CastJuiceProfile profile;
        boolean fired;

        Pending(Identity id, CastJuiceProfile profile) {
            this.id = id;
            this.profile = profile;
        }
    }
}
