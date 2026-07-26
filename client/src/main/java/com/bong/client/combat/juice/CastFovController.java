package com.bong.client.combat.juice;

import com.bong.client.animation.BongAnimations;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.DeathStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientEntityEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.entity.Entity;
import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.LongSupplier;

/**
 * plan-fpv-cast-av-v1 P3 —— 施法瞬间 juice：FOV 加法脉冲控制器 + cast 状态转换驱动的调度。
 *
 * <p><b>生命周期契约</b>（§P3）：状态机 {@code idle → pulse → decay → idle}，所有路径终点回到
 * 单一基准 FOV（加法偏移归零），复位幂等。client tick 推进 decay 与死亡 teardown。
 *
 * <p><b>两条驱动路径</b>（都只产出同一个 {@link #pulse} / 共享 shake 单通道，last-write-wins）：
 * <ol>
 *   <li><b>cast 状态转换</b>——{@link CastStateStore} 的回调（由 {@code CastSyncHandler} /
 *       本地预测 replace 触发），走下述 identity 门控。绝大多数重型招走这条。</li>
 *   <li><b>动画事件</b>——{@link #onAnimPlayed}（{@code VfxEventRouter} 在 play_anim 真正播出后调）。
 *       heaven_gate 专用：它的 cast 条与真实引导窗错开 3s，只有动画事件能对准劈下那一刻，详见
 *       下方「动画事件驱动的 juice」段。</li>
 * </ol>
 *
 * <p><b>门控</b>（§8.1 #3）：release juice 只绑**已 accepted（观测到 CASTING）的 cast identity**。
 * identity = {@code (slot, startedAtMs)} 二元组（{@link CastState} 无唯一 cast id；不含推断得来的
 * source，理由见 {@link Identity}）。
 * <ul>
 *   <li>CASTING：按 identity 武装 pending（该招在 {@link CastJuiceProfiles} 有 profile 才武装）；
 *       同 identity 重复 CASTING 幂等，异 identity 取代旧 pending（supersession）。</li>
 *   <li>COMPLETE：identity 匹配且未触发过 → 触发 FOV 脉冲 + shake，标记已触发（同 identity 重复
 *       release 幂等）。</li>
 *   <li>INTERRUPT：只作废**同 identity** 的 pending（异 identity 的在飞 cast 不受牵连），并记录
 *       被作废 identity（防**乱序**——打断早于 CASTING 到达时，后到的同 identity CASTING 不再武装）。</li>
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
    /**
     * pending / voidedId / pulse **以及 shake 通道写入**的互斥锁（cast 转换回调、动画事件、
     * teardown、倍率变更可能分别落在网络线程与主线程）。两个 juice 通道的建立与清空都串行化
     * 在这里，避免交错留下孤儿抖动。{@link CameraShakeController} 本身无锁，故不存在锁序倒置。
     */
    private static final Object LOCK = new Object();

    /** 施法 release 抖动方向（自施法无攻击方向，取对角 → 偏航+俯仰混合抖动）。 */
    private static final double CAST_SHAKE_DIR_X = 1.0;
    private static final double CAST_SHAKE_DIR_Z = 1.0;

    /** 时钟 seam：生产 = {@code System.currentTimeMillis}；单测注入可推进时钟。 */
    private static volatile LongSupplier clock = System::currentTimeMillis;

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
        // 切世界（跨维度 / 换服）：vanilla 不重建 ClientPlayNetworkHandler，只发 PlayerRespawn
        // 换掉 ClientWorld，故 DISCONNECT / JOIN 都**不触发**。Fabric 在 onPlayerRespawn、
        // onGameJoin、clearWorld 三处对**旧世界**全量 emit ENTITY_UNLOAD，本地玩家实体被卸载
        // 即「旧世界整体拆除」——这是 1.20.1 Fabric API 里能覆盖切世界的唯一现成钩子
        //（该版本无 ClientWorldEvents）。
        ClientEntityEvents.ENTITY_UNLOAD.register((entity, world) -> onEntityUnload(entity));
    }

    /**
     * 实体从 {@code ClientWorld} 卸载。<b>本地玩家实体</b>被卸载 = 旧世界整体拆除（切维度 /
     * 换服 / 断线）→ 立即 {@link #teardown()}：否则跨世界后旧脉冲会继续叠 FOV、旧 pending 会
     * 被迟到的 complete 触发，juice 跨世界残留。远端玩家/怪物的卸载（走 stopTracking）不动 juice。
     */
    static void onEntityUnload(Entity entity) {
        if (localPlayerEntityPredicate.isLocalPlayerEntity(entity)) {
            teardown();
        }
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

    /**
     * INTERRUPT：作废<b>该 identity 自己的</b> pending。
     *
     * <p>{@link #voidedId} 无条件记录（防乱序：打断早于 CASTING 到达时，后到的同 identity
     * 不再武装）；但 {@link #pending} <b>只在身份匹配时</b>才清——否则 A 被 B 取代后，一条
     * 迟到/重传的 {@code INTERRUPT(A)} 会顺手把 B 的 pending 误杀，B 随后 release 静默无 juice。
     * 取消令牌是绑 identity 的，不是「见到打断就清场」。
     */
    private static void voidPending(CastState state) {
        Identity id = Identity.of(state);
        voidedId = id;
        if (pending != null && pending.id.equals(id)) {
            pending = null;
        }
    }

    /**
     * 触发 FOV 脉冲 + shake（同帧调度）。倍率在触发时刻并入<b>两个通道</b>（脉冲峰值 / shake
     * 强度）；倍率 0 = 全局关闭 → 直接不触发，连脉冲对象都不建——否则会留下一个「读数被遮蔽
     * 但还在时间窗内」的僵尸脉冲，玩家把倍率调回来时它会诈尸。
     */
    private static void fire(CastJuiceProfile profile, long now) {
        float scale = JuiceConfig.juiceMultiplier();
        if (scale <= 0f) {
            return;  // juice 全局关闭
        }
        if (profile.hasFovPulse()) {
            pulse = new Pulse(profile.fovPeakDegrees() * scale, profile.fovDurationTicks(), now);
        }
        if (profile.hasShake()) {
            // 施法 release = 持续震动（SUSTAIN 包络）：把大招的存在感撑满整个时长，
            // 不是命中的「抖一下」线性衰减。
            CameraShakeController.triggerDirect(
                profile.shakeIntensity() * scale,
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
     * 卸载实体是否本地玩家的判定 seam（单测无法构造 {@code ClientPlayerEntity}——需要完整
     * world/registry 环境，且本仓库 client 测试无 mock 框架）。
     */
    @FunctionalInterface
    public interface LocalPlayerEntityPredicate {
        boolean isLocalPlayerEntity(Entity entity);
    }

    /**
     * 生产判定：{@code ClientWorld} 里唯一的 {@link ClientPlayerEntity} 就是本地玩家
     *（远端玩家是 {@code OtherClientPlayerEntity}）。{@code instanceof} 天然 null-safe。
     */
    private static volatile LocalPlayerEntityPredicate localPlayerEntityPredicate =
        entity -> entity instanceof ClientPlayerEntity;

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
        float scale = JuiceConfig.juiceMultiplier();
        if (scale <= 0f) {
            return;  // juice 全局关闭：与 fire() 同策，不留僵尸脉冲/抖动
        }
        long now = clock.getAsLong();
        ChargeShake charge = CHARGE_ANIM_JUICE.get(animId);
        if (charge != null) {
            synchronized (LOCK) {
                CameraShakeController.triggerDirect(
                    charge.peakIntensity() * scale,
                    charge.buildDurationTicks(),
                    CAST_SHAKE_DIR_X,
                    CAST_SHAKE_DIR_Z,
                    false,
                    CameraShakeController.Envelope.CRESCENDO,
                    now
                );
            }
            return;
        }
        ReleaseBurst burst = RELEASE_ANIM_JUICE.get(animId);
        if (burst != null) {
            // 两个通道在同一临界区落地，与 teardown / onJuiceMultiplierChanged 的清空对称——
            // 否则「清脉冲…清抖动」与「建脉冲…起抖动」交错时会漏下一条孤儿抖动。
            synchronized (LOCK) {
                if (burst.fovPeakDegrees() > 0f && burst.fovDurationTicks() > 0) {
                    pulse = new Pulse(burst.fovPeakDegrees() * scale, burst.fovDurationTicks(), now);
                }
                CameraShakeController.triggerDirect(
                    burst.shakeIntensity() * scale,
                    burst.shakeDurationTicks(),
                    CAST_SHAKE_DIR_X,
                    CAST_SHAKE_DIR_Z,
                    false,
                    CameraShakeController.Envelope.SUSTAIN,
                    now
                );
            }
        }
    }

    private static boolean isLocalPlayerFromClient(UUID playerId) {
        MinecraftClient client = MinecraftClient.getInstance();
        return client != null && client.player != null
            && client.player.getUuid().equals(playerId);
    }

    /**
     * 渲染线程每帧调（{@code MixinGameRenderer}）：当前加法 FOV 偏移（0 = 基准）。倍率已在
     * {@link #fire} 时刻烘焙进 {@link Pulse#peakDegrees}，此处不再乘——倍率调 0 靠
     * {@link JuiceConfig#setJuiceMultiplier} → {@link #onJuiceMultiplierChanged} 真正清空脉冲，
     * 而不是把读数遮掉。
     */
    public static double fovDelta() {
        Pulse p = pulse;
        return p == null ? 0.0 : p.offsetAt(clock.getAsLong());
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
        // 抖动与脉冲同处一个临界区：两个通道的写入都串行化在 LOCK 上，否则网络线程的 fire
        // 可能恰好落在「清脉冲」与「清抖动」之间，留下一条孤儿抖动。
        synchronized (LOCK) {
            pulse = null;
            pending = null;
            voidedId = null;
            // 死亡/断线也清抖动：蓄力 CRESCENDO 长达数秒，玩家已死不应继续震屏。
            CameraShakeController.clear();
        }
    }

    /**
     * 倍率变更传播（唯一调用方 {@link JuiceConfig#setJuiceMultiplier}，写入即调，不经 bootstrap
     * 注册，避免漏注册变成静默孤岛）。
     *
     * <p>plan §P3「<b>进行中把倍率调 0 立即复位</b>，不是只影响后续脉冲」：转 0 时走与
     * {@link #teardown()} 同一条<b>受控取消</b>路径——清当前 FOV 脉冲（回基准）<b>并且</b>
     * {@link CameraShakeController#clear()} 停当前抖动。两个通道都真停，不是只把 FOV 读数
     * 遮成 0 而让抖动继续、让脉冲状态苟活。
     *
     * <p>取消是<b>不可逆</b>的：恢复倍率只影响后续 release，旧脉冲已被清空、无从复活
     *（在播 juice 的强度是 {@link #fire} 时刻烘焙的，本来也不追溯）。幂等。
     *
     * <p>抖动是与命中 juice 共享的单通道，故关 juice 时在播的命中抖动一并停——与
     * {@link #teardown()} 同款取舍：玩家把 juice 关了就该立刻安静。
     */
    public static void onJuiceMultiplierChanged(float value) {
        if (value > 0f) {
            return;
        }
        // 两个通道同锁清空，避免关闭瞬间被随后落地的 fire 复活一帧、或留下孤儿抖动。
        synchronized (LOCK) {
            pulse = null;
            CameraShakeController.clear();
        }
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

    /**
     * 动画事件驱动 juice 的本地玩家判定 seam（单测注入，免 MinecraftClient）。
     * public 与 {@link CameraShakeController#resetForTests()} 同例——跨包的路由接线测试
     *（{@code com.bong.client.network.VfxEventRouterTest}）要验 play_anim → juice 这条链。
     */
    public static void setLocalPlayerPredicateForTest(LocalPlayerPredicate p) {
        localPlayerPredicate = p == null ? CastFovController::isLocalPlayerFromClient : p;
    }

    /** ENTITY_UNLOAD 本地玩家实体判定 seam（单测注入，免构造 ClientPlayerEntity）。 */
    static void setLocalPlayerEntityPredicateForTest(LocalPlayerEntityPredicate p) {
        localPlayerEntityPredicate = p == null ? (e -> e instanceof ClientPlayerEntity) : p;
    }

    /** public 理由同 {@link #setLocalPlayerPredicateForTest}（跨包路由接线测试要复位 juice 状态）。 */
    public static void resetForTests() {
        pulse = null;
        synchronized (LOCK) {
            pending = null;
            voidedId = null;
        }
        JuiceConfig.resetForTests();
        clock = System::currentTimeMillis;
        localPlayerPredicate = CastFovController::isLocalPlayerFromClient;
        localPlayerEntityPredicate = entity -> entity instanceof ClientPlayerEntity;
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

    /**
     * cast 身份 = {@code (slot, startedAtMs)}（{@link CastState} 无唯一 cast id）。
     *
     * <p><b>刻意不含 {@code source}</b>：source 不是 wire 字段，{@code CastSyncHandler.sourceFor}
     * 靠「当前快照是否正 CASTING 在同一 slot」<b>推断</b>它，任何非 CASTING 快照（如一条迟到的
     * INTERRUPT 落地后）都会让后续回执退化成 QUICK_SLOT。把这个会丢失的推断值写进身份，会让
     * 「A 被 B 取代 → 迟到 INTERRUPT(A) → COMPLETE(B)」里的 COMPLETE(B) 认不出自己的 pending，
     * 于是 juice 静默不触发。{@code (slot, startedAtMs)} 是权威 wire 字段，不受推断污染。
     *
     * <p>source 的门控作用保留在 <b>arming 时刻</b>：{@link #resolveSkillId} 只给 SKILL_BAR
     * 的 cast 解析 profile，故 pending 天然只属于技能栏施法。同 slot 同毫秒起手的两次不同
     * 来源施法在物理上不可能（玩家一次只能起一个 cast），身份不会撞。
     */
    private record Identity(int slot, long startedAtMs) {
        static Identity of(CastState s) {
            return new Identity(s.slot(), s.startedAtMs());
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
