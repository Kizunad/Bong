package com.bong.client.combat.juice;

import com.bong.client.animation.BongAnimations;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.DeathStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.UUID;
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

    /**
     * 全局 juice 强度倍率（0 = 关闭），默认 1.0。**FOV 分量**每帧在 {@link #fovDelta()} 里乘它
     * → 进行中调 0 时 FOV 即时归基准（持久可见分量，测试断言的对象）。**shake 分量**在
     * {@link #fire} 触发时刻并入强度（走共享单通道无法追溯缩放；短抖动 ≤400ms 自然播完）。
     */
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
            // 施法 release = 持续震动（SUSTAIN 包络）：把大招的存在感撑满整个时长，
            // 不是命中的「抖一下」线性衰减。倍率在触发时刻并入强度（0 → 不触发）。
            CameraShakeController.triggerDirect(
                profile.shakeIntensity() * multiplier,
                profile.shakeDurationTicks(),
                CAST_SHAKE_DIR_X,
                CAST_SHAKE_DIR_Z,
                false,
                CameraShakeController.Envelope.SUSTAIN,
                now
            );
        }
    }

    // ─── 动画事件驱动的 juice（heaven_gate 专用，与 cast 条脱钩）─────────────────
    //
    // heaven_gate 的 cast_ticks(80=4s) 与真实引导窗（HeavenGateChanneling 到
    // HEAVEN_GATE_AOE_END=140=7s 才 emit release）错开 3s，走 CastState 驱动会让 juice 在
    // cast 条走完（4s、举剑蓄力中途）就触发、而非劈下那一刻（7s）。故 heaven_gate 从
    // CastJuiceProfiles（CastState 驱动）移除，改由**动画事件**驱动：charge 动画起播 →
    // CRESCENDO 渐强震动；release 动画（劈下）起播 → SUSTAIN 最大震动 + FOV punch。二者
    // 都是 server 在视觉正确时刻发的 PlayAnim，故 juice 与画面严格对齐。

    /** 蓄力段动画 → 渐强震动（CRESCENDO：0→peak 爬满后维持，撑到 release 顶替）。 */
    private record ChargeShake(float peakIntensity, int buildDurationTicks) {
    }

    /** 释放段动画 → 最大震动（SUSTAIN）+ FOV punch，落在劈下那一刻。 */
    private record ReleaseBurst(
        float shakeIntensity, int shakeDurationTicks, float fovPeakDegrees, int fovDurationTicks) {
    }

    /** 蓄力渐强震动 buildDuration=160t：ramp 到峰值 ~144t，覆盖 release(140t)+RTT 余量。 */
    private static final Map<Identifier, ChargeShake> CHARGE_ANIM_JUICE = Map.of(
        BongAnimations.SWORD_HEAVEN_GATE_CHARGE, new ChargeShake(0.8f, 160));

    /** release 最大震动 24t（≈1.2s SUSTAIN）+ FOV +12°/8t punch。 */
    private static final Map<Identifier, ReleaseBurst> RELEASE_ANIM_JUICE = Map.of(
        BongAnimations.SWORD_HEAVEN_GATE_RELEASE, new ReleaseBurst(1.5f, 24, 12.0f, 8));

    /** 本地玩家判定 seam（动画事件驱动的 juice 只对本地玩家自己的施法触发）。 */
    @FunctionalInterface
    public interface LocalPlayerPredicate {
        boolean isLocal(UUID playerId);
    }

    private static volatile LocalPlayerPredicate localPlayerPredicate =
        CastFovController::isLocalPlayerFromClient;

    /**
     * 动画事件驱动 juice 入口（{@code VfxEventRouter} 在 PlayAnim 分支调）：只对**本地玩家
     * 自己**的登记动画触发。charge 动画 → CRESCENDO 渐强震动；release 动画（劈下那一刻）→
     * SUSTAIN 最大震动 + FOV 脉冲。二者共享 {@link CameraShakeController} 单通道，release 的
     * SUSTAIN 顶替 charge 的 CRESCENDO。非本地玩家 / 非登记动画 = no-op。
     */
    public static void onAnimPlayed(UUID targetPlayer, Identifier animId) {
        if (targetPlayer == null || animId == null || !localPlayerPredicate.isLocal(targetPlayer)) {
            return;
        }
        long now = clock.getAsLong();
        ChargeShake charge = CHARGE_ANIM_JUICE.get(animId);
        if (charge != null) {
            CameraShakeController.triggerDirect(
                charge.peakIntensity() * multiplier,
                charge.buildDurationTicks(),
                CAST_SHAKE_DIR_X,
                CAST_SHAKE_DIR_Z,
                false,
                CameraShakeController.Envelope.CRESCENDO,
                now
            );
            return;
        }
        ReleaseBurst burst = RELEASE_ANIM_JUICE.get(animId);
        if (burst != null) {
            if (burst.fovPeakDegrees() > 0f && burst.fovDurationTicks() > 0) {
                synchronized (LOCK) {
                    pulse = new Pulse(burst.fovPeakDegrees(), burst.fovDurationTicks(), now);
                }
            }
            CameraShakeController.triggerDirect(
                burst.shakeIntensity() * multiplier,
                burst.shakeDurationTicks(),
                CAST_SHAKE_DIR_X,
                CAST_SHAKE_DIR_Z,
                false,
                CameraShakeController.Envelope.SUSTAIN,
                now
            );
        }
    }

    private static boolean isLocalPlayerFromClient(UUID playerId) {
        MinecraftClient client = MinecraftClient.getInstance();
        return client != null && client.player != null
            && client.player.getUuid().equals(playerId);
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
        // pulse 的 check-then-clear 与 fire() 的写入共用 LOCK（fire 在 onCastState 的
        // synchronized 内跑）——否则网络线程恰在本处检查与清空之间 fire 新脉冲，主线程会误清
        // 刚武装的脉冲（KillJuiceController.activeKill 同款同锁模式）。fovDelta 的 volatile 读无锁。
        synchronized (LOCK) {
            Pulse p = pulse;
            if (p != null && !p.activeAt(clock.getAsLong())) {
                pulse = null;  // 脉冲自然结束 → 回 idle
            }
        }
    }

    /** 断线 / 切世界 / 死亡：立即复位基准 FOV + 清 pending + 清抖动（幂等，重复调用无副作用）。 */
    public static void teardown() {
        // pulse 清空与 fire() 写入同锁，避免死亡/断线 teardown 被随后落地的 fire 复活一帧。
        synchronized (LOCK) {
            pulse = null;
            pending = null;
            voidedId = null;
        }
        // 死亡/断线也清抖动：蓄力 CRESCENDO 长达数秒，玩家已死不应继续震屏。
        CameraShakeController.clear();
    }

    /**
     * 设全局 juice 强度倍率（0 = 关闭）。FOV 分量即时复位（{@link #fovDelta} 每帧乘它）；
     * shake 分量下次 {@link #fire} 时并入（已在播的短抖动自然播完）。见 {@link #multiplier}。
     */
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

    /** 动画事件驱动 juice 的本地玩家判定 seam（单测注入，免 MinecraftClient）。 */
    static void setLocalPlayerPredicateForTest(LocalPlayerPredicate p) {
        localPlayerPredicate = p == null ? CastFovController::isLocalPlayerFromClient : p;
    }

    static void resetForTests() {
        pulse = null;
        synchronized (LOCK) {
            pending = null;
            voidedId = null;
        }
        multiplier = 1.0f;
        clock = System::currentTimeMillis;
        localPlayerPredicate = CastFovController::isLocalPlayerFromClient;
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
