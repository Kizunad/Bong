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

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
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
 *       heaven_gate 专用：它的 cast 条与真实引导窗错开 3s，只有动画事件能对准劈下那一刻。
 *       <b>动画事件只决定触发时刻，授权仍来自权威 CASTING 武装的 {@link AnimCastToken}</b>，
 *       与路径 1 同一套 identity 门控与终态记录，详见下方「动画事件驱动的 juice」段。</li>
 * </ol>
 *
 * <p><b>门控</b>（§8.1 #3）：release juice 只绑**已 accepted 的 cast identity**。
 * identity = {@code (slot, startedAtMs)} 二元组（{@link CastState} 无唯一 cast id；不含推断得来的
 * source，理由见 {@link Identity}）。
 *
 * <p><b>「accepted」= 服务端权威回执，不是本地预测</b>（{@link CastStateStore.Origin}）：
 * {@code SkillBarKeyRouter} 在**按键那一刻**就 {@code beginSkillBarCast} 写出一条 CASTING
 * （纯乐观预测，服务端还没回话），{@code CastStateStore.tick} 也会按本地计时把它推到 COMPLETE。
 * 只有 {@code CastSyncHandler} → {@link CastStateStore#replace} 落地的 CASTING 才是
 * {@link CastStateStore.Origin#SERVER_AUTHORITATIVE}，也才允许武装。本地预测最多是**候选**：
 * 它只影响 {@code CastSyncHandler.sourceFor} 的 SKILL_BAR 推断（见 {@link #resolveSkillId}），
 * 不授予任何触发权。**服务端拒绝 / 压根没接受的施法，本地计时到点也零 juice。**
 * <ul>
 *   <li>CASTING（仅权威）：按 identity 武装 pending（该招在 {@link CastJuiceProfiles} 有 profile
 *       才武装）；同 identity 重复 CASTING 幂等，异 identity 取代旧 pending（supersession）。
 *       预测来源的 CASTING 一律不武装。</li>
 *   <li>COMPLETE：identity 匹配 → 触发 FOV 脉冲 + shake，并把该 identity 记成
 *       {@link Terminal#FIRED}（同 identity 重复 release / 跨 IDLE 重放都不再触发）。</li>
 *   <li>INTERRUPT：把该 identity 记成 {@link Terminal#VOIDED}，但只清**同 identity** 的
 *       pending（异 identity 的在飞 cast 不受牵连）。</li>
 *   <li>IDLE：<b>no-op</b>——{@link CastState#idle()} 是无身份单例（slot=-1、startedAtMs=0），
 *       无法归属到任何 cast，无条件清场会让一条迟到的 IDLE 误杀在飞的施法。详见 {@link #onCastState}。</li>
 * </ul>
 *
 * <p><b>终态记录（{@link #terminals}）</b>：幂等与防复活不能只靠「当前 pending 里的一个 fired 标记
 * + 一个单值 voidedId」——前者随 pending 一起被 supersession/IDLE 丢掉（同 identity 迟到 CASTING
 * 能重建 pending 再触发第二次），后者被下一次 INTERRUPT 覆盖（{@code INTERRUPT(A)→INTERRUPT(B)→
 * CASTING(A)} 让 A 复活）。故按 identity 存**有界**（LRU {@link #TERMINAL_MEMORY}）终态记录，
 * 区分 FIRED / VOIDED，先到者胜（已 FIRED 的不因迟到打断被改写）。teardown 把当时在飞的身份
 * 整批记成 VOIDED，故 teardown 之后迟到的旧 CASTING/COMPLETE 不再复活。
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
     * pending / {@link #terminals} / pulse **以及 shake 通道写入**的互斥锁。两个 juice 通道的
     * 建立与清空都串行化在这里，避免交错留下孤儿抖动。
     *
     * <p>网络侧回执当前由 {@code BongNetworkHandler} 经 {@code client.execute} 全部 marshal 到
     * 客户端线程，故实际竞争极少；但渲染线程会并发读 {@link #pulse}（{@link #fovDelta()} 走
     * volatile 无锁读），且这里是唯一保证「两通道原子成对」的地方，故仍显式加锁。
     * {@link CameraShakeController} / {@link JuiceConfig} / {@link SkillBarStore} 均为无锁
     * volatile 读写，锁内不调任何会再取监视器的东西，故不存在锁序倒置。
     */
    private static final Object LOCK = new Object();

    /** 施法 release 抖动方向（自施法无攻击方向，取对角 → 偏航+俯仰混合抖动）。 */
    private static final double CAST_SHAKE_DIR_X = 1.0;
    private static final double CAST_SHAKE_DIR_Z = 1.0;

    /**
     * 天门开阖 skillId（对齐服务端 {@code sword_path::skill_register::SWORD_PATH_HEAVEN_GATE_ID}
     * 与 client {@code SkillBarEntry.id()}）——唯一走动画事件驱动的招，见
     * {@link #ANIM_DRIVEN_SKILLS}。
     */
    static final String HEAVEN_GATE_SKILL_ID = "sword_path.heaven_gate";

    /** 时钟 seam：生产 = {@code System.currentTimeMillis}；单测注入可推进时钟。 */
    private static volatile LongSupplier clock = System::currentTimeMillis;

    /** 当前 FOV 脉冲（不可变；volatile 供渲染线程读）。null = idle（基准 FOV）。 */
    private static volatile Pulse pulse = null;

    /** 当前武装的 pending juice（LOCK 保护）。 */
    private static Pending pending = null;

    /** cast identity 的终态：已触发过 / 已作废。两者都禁止该 identity 再次武装或触发。 */
    enum Terminal { FIRED, VOIDED }

    /**
     * 终态记录容量（LRU 上限，LOCK 保护）。乱序窗口是网络重传/回执错序的量级（个位数施法），
     * 16 条远超实际需要；有界是硬要求——无界 Map 会随会话时长单调增长。
     *
     * <p>package-private 供有界性测试引用，免得测试另抄一份 16（改容量时测试要跟着一起改）。
     */
    static final int TERMINAL_MEMORY = 16;

    /**
     * identity → 终态（LOCK 保护）。{@link LinkedHashMap} 访问序 + {@code removeEldestEntry}
     * 做 LRU：满了先淘汰最久未被**写入或 get** 的身份（最不可能再有迟到回执的那个）。
     *
     * <p>注意 {@code containsKey} <b>不</b>刷新访问序（只有 {@code get}/{@code put} 系列会），
     * 故 {@link #arm} 的终态查询不会把老身份「续命」。这对有界性无影响，实际逐出顺序≈写入序。
     */
    private static final LinkedHashMap<Identity, Terminal> terminals =
        new LinkedHashMap<>(TERMINAL_MEMORY * 2, 0.75f, true) {
            @Override
            protected boolean removeEldestEntry(Map.Entry<Identity, Terminal> eldest) {
                return size() > TERMINAL_MEMORY;
            }
        };

    private CastFovController() {
    }

    /** 客户端启动挂钩：cast 转换监听 + tick decay + 断线/切世界 teardown。 */
    public static void bootstrap() {
        if (!BOOTSTRAPPED.compareAndSet(false, true)) {
            return;
        }
        // 必须挂**带来源**的监听：Consumer 版拿不到 Origin，会把本地预测当成 accepted。
        CastStateStore.addTransitionListener(CastFovController::onCastState);
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
     *
     * <p><b>只有 {@link CastStateStore.Origin#SERVER_AUTHORITATIVE} 的 CASTING 才武装</b>——
     * 预测来源的 CASTING 只是候选（见类文档「accepted」段）。COMPLETE 不看来源：它只是**触发
     * 时刻**信号，权限来自那条已被权威武装的 pending；缺 pending（= 未 accepted 或已作废）
     * 一律 no-op，故「按键预测 → 本地计时到点 → COMPLETE」拿不到 juice。
     *
     * <p><b>IDLE 是 no-op</b>：{@link CastState#idle()} 是无身份单例（{@code slot=-1}、
     * {@code startedAtMs=0}），无法归属到任何一次施法，因此「见 IDLE 就清 pending」必然是
     * 跨身份清场——A 被 B 取代后一条迟到的 IDLE(A) 会误杀 B，B 随后 release 静默无 juice。
     * 也没有必须靠 IDLE 兜的路径：生产上 IDLE 只有两个来源，① {@code CastStateStore.tick}
     * 在 COMPLETE/INTERRUPT 后 300ms 的淡出（此时该身份已是 FIRED/VOIDED 终态），②
     * 服务端 {@code cast_sync{phase:idle}}——而服务端的 idle 一律带 {@code Reject*} outcome
     * （{@code push_skill_cast_rejected_sync} 等三处），{@code CastSyncHandler} 会把它合成
     * 成 INTERRUPT 走取消令牌，不会以 IDLE 形态到这里。跨 IDLE 的幂等由 {@link #terminals}
     * 保证，不靠清场。
     */
    static void onCastState(CastState state, CastStateStore.Origin origin) {
        if (state == null) {
            return;
        }
        long now = clock.getAsLong();
        synchronized (LOCK) {
            switch (state.phase()) {
                case CASTING -> {
                    if (origin == CastStateStore.Origin.SERVER_AUTHORITATIVE) {
                        arm(state, now);
                    }
                }
                case COMPLETE -> release(state, now);
                case INTERRUPT -> voidCast(state);
                case IDLE -> { }  // 无身份，不清场（理由见方法文档）
            }
        }
    }

    /**
     * 权威 CASTING → 武装。按招式分两条：走动画事件驱动的招（{@link #ANIM_DRIVEN_SKILLS}）
     * 发一枚 {@link AnimCastToken}（触发时刻由 {@link #onAnimPlayed} 决定），其余重型招按
     * {@link CastJuiceProfiles} 武装 pending（触发时刻是 COMPLETE）。
     */
    private static void arm(CastState state, long now) {
        Identity id = Identity.of(state);
        if (terminals.containsKey(id)) {
            return;  // 该身份已终态（FIRED 不重放 / VOIDED 不复活），含乱序打断早到与 teardown
        }
        // supersession：异身份的新施法取代旧 pending（该通道只留最新一次施法）
        if (pending != null && !pending.id().equals(id)) {
            pending = null;
        }
        String skillId = resolveSkillId(state);
        if (skillId == null) {
            return;  // 非技能栏施法 / 空槽 → 无 juice
        }
        AnimDrivenJuice animJuice = animDrivenJuice(skillId);
        if (animJuice != null) {
            // 令牌只被**另一次动画事件驱动的施法**取代，不被无关招式的 CASTING 顶掉：
            // heaven_gate 的 cast 条 4s 就走完（服务端随即 remove Casting），而 release 动画要到
            // 引导第 140t（7s）才发——中间这 3s 玩家放任何别的招都会带来一条权威 CASTING，
            // 若那也算 supersession，天门劈下那一刻会静默丢掉 juice。该令牌的结束条件只有：
            // 自己的 release 消费掉 / 同身份 INTERRUPT / teardown / TTL 过期 / 另一次动画驱动施法。
            if (animToken != null && !animToken.id.equals(id)) {
                animToken = null;
            }
            if (animToken == null) {
                animToken = new AnimCastToken(id, animJuice, now);  // 同身份重复 CASTING：幂等
            }
            return;  // 该招不由 COMPLETE 触发，只发令牌
        }
        if (pending != null) {
            return;  // 同身份重复 CASTING：幂等
        }
        CastJuiceProfile profile = CastJuiceProfiles.get(skillId);
        if (profile != null) {
            pending = new Pending(id, profile);
        }
    }

    private static void release(CastState state, long now) {
        Identity id = Identity.of(state);
        Pending armed = pending;
        if (armed == null || !armed.id().equals(id)) {
            return;  // 无 pending（未 accepted / 已作废 / 已触发过）或身份不符
        }
        // 消费掉 pending 并落 FIRED 终态：同 identity 的重复 / 乱序 / 跨 IDLE 重放都不再触发。
        pending = null;
        markTerminal(id, Terminal.FIRED);
        fire(armed.profile(), now);
    }

    /**
     * INTERRUPT：把该 identity 记成 {@link Terminal#VOIDED}，并作废<b>该 identity 自己的</b> pending。
     *
     * <p>终态**无条件**记录（防乱序：打断早于 CASTING 到达时，后到的同 identity 不再武装）；
     * 但 {@link #pending} <b>只在身份匹配时</b>才清——否则 A 被 B 取代后，一条迟到/重传的
     * {@code INTERRUPT(A)} 会顺手把 B 的 pending 误杀，B 随后 release 静默无 juice。取消令牌
     * 是绑 identity 的，不是「见到打断就清场」。
     */
    private static void voidCast(CastState state) {
        Identity id = Identity.of(state);
        markTerminal(id, Terminal.VOIDED);
        if (pending != null && pending.id().equals(id)) {
            pending = null;
        }
        if (animToken != null && animToken.id.equals(id)) {
            animToken = null;  // 取消后迟到的 charge/release 动画不再授权（review finding A）
        }
    }

    /** 终态先到者胜：已 FIRED 的身份不因一条迟到的打断被改写成 VOIDED（LOCK 内调用）。 */
    private static void markTerminal(Identity id, Terminal terminal) {
        terminals.putIfAbsent(id, terminal);
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
            triggerShake(profile.shakeIntensity() * scale, profile.shakeDurationTicks(),
                CameraShakeController.Envelope.SUSTAIN, now);
        }
    }

    // ─── 动画事件驱动的 juice（heaven_gate 专用，与 cast 条脱钩）─────────────────
    //
    // heaven_gate 的 cast_ticks(80=4s) 与真实引导窗（HeavenGateChanneling 到
    // HEAVEN_GATE_AOE_END=140=7s 才 emit release）错开 3s，走 CastState 驱动会让 juice 在
    // cast 条走完（4s、举剑蓄力中途）就触发、而非劈下那一刻（7s）。故 heaven_gate 从
    // CastJuiceProfiles（CastState 驱动）移除，改由**动画事件**驱动：charge 动画起播 →
    // CRESCENDO 渐强震动；release 动画（劈下）起播 → SUSTAIN 最大震动 + FOV punch。
    //
    // ⚠️ 动画事件只是**触发时刻**信号，不是授权。授权凭据仍是权威 CASTING 武装出来的
    // AnimCastToken（见 #animToken）——bridge 播出成功只证明「这段动画播了」，不证明
    // 「这次施法被服务端 accepted / 尚未被取消 / 尚未触发过」。

    /** 蓄力段动画 → 渐强震动（CRESCENDO：0→peak 爬满后维持）。 */
    record ChargeShake(
        float peakIntensity, int buildDurationTicks, CameraShakeController.Envelope envelope) {
    }

    /** 释放段动画 → 最大震动（SUSTAIN）+ FOV punch，落在劈下那一刻。 */
    record ReleaseBurst(
        float shakeIntensity,
        int shakeDurationTicks,
        float fovPeakDegrees,
        int fovDurationTicks,
        CameraShakeController.Envelope envelope
    ) {
    }

    /**
     * 蓄力渐强震动 buildDuration=60t：对齐 {@code sword_heaven_gate_charge} 动画自身的
     * {@code endTick=60}（{@code stopTick=64}）——震感跟着**在播的蓄力动画**渐强，动画淡出即止。
     *
     * <p>不取 release(140t) 那么长：charge 动画只播 0→60t，之后淡回默认站姿；把震感撑到 140t
     * 会让「画面上没有蓄力动作了却还在震」，违背本条 juice「与画面严格对齐」的前提。charge
     * 动画时长本身归 P2（动画资产阶段），本 plan P3 只跟随它取值。
     */
    private static final ChargeShake HEAVEN_GATE_CHARGE_JUICE =
        new ChargeShake(0.8f, 60, CameraShakeController.Envelope.CRESCENDO);

    /** release 最大震动 24t（≈1.2s SUSTAIN）+ FOV +12°/8t punch。 */
    private static final ReleaseBurst HEAVEN_GATE_RELEASE_JUICE =
        new ReleaseBurst(1.5f, 24, 12.0f, 8, CameraShakeController.Envelope.SUSTAIN);

    /**
     * 动画事件驱动招式的 juice 契约：该招的 charge / release 动画 id + 两段参数。
     *
     * <p>动画 id 与 skillId 一起登记，是「动画事件如何无歧义关联到 cast identity」的答案：
     * 令牌由该 skillId 的权威 CASTING 武装，只有**这一对** animId 能消费它。别的招的动画
     * 到达时找不到匹配令牌 → no-op，不会误用当前令牌。
     */
    private record AnimDrivenJuice(
        Identifier chargeAnim, Identifier releaseAnim, ChargeShake charge, ReleaseBurst release) {
    }

    /** 走动画事件驱动的招（skillId → 契约）。当前仅 heaven_gate（理由见上方段注释）。 */
    private static final Map<String, AnimDrivenJuice> ANIM_DRIVEN_SKILLS = Map.of(
        HEAVEN_GATE_SKILL_ID,
        new AnimDrivenJuice(
            BongAnimations.SWORD_HEAVEN_GATE_CHARGE,
            BongAnimations.SWORD_HEAVEN_GATE_RELEASE,
            HEAVEN_GATE_CHARGE_JUICE,
            HEAVEN_GATE_RELEASE_JUICE));

    /**
     * 令牌存活上限（ms）。heaven_gate 的 release 动画在引导第 140t（7s）才发，故窗口必须
     * 宽于 7s；15s 留足 RTT / 卡顿余量。有上限是硬要求：release 动画因丢包/异常始终没来时，
     * 令牌不能一直挂着等一条几分钟后到达的杂散 PlayAnim 把它消费掉。
     */
    static final long ANIM_TOKEN_TTL_MS = 15_000L;

    /**
     * 当前 heaven_gate 施法的动画 juice 令牌（LOCK 保护）。由**权威 CASTING** 武装，
     * charge/release 动画各自一次性消费；INTERRUPT / teardown 按身份作废；异身份的权威
     * CASTING 取代（supersession）；超过 {@link #ANIM_TOKEN_TTL_MS} 视为过期。
     */
    private static AnimCastToken animToken = null;

    /** 动画事件驱动 juice 的授权令牌：绑 cast identity + 两段各自的一次性消费标记。 */
    private static final class AnimCastToken {
        final Identity id;
        final AnimDrivenJuice juice;
        final long armedAtMs;
        boolean chargeFired;
        boolean releaseFired;

        AnimCastToken(Identity id, AnimDrivenJuice juice, long armedAtMs) {
            this.id = id;
            this.juice = juice;
            this.armedAtMs = armedAtMs;
        }
    }

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
     * 动画事件驱动 juice 入口（{@code VfxEventRouter} 在 PlayAnim 真正播出后调）。
     *
     * <p><b>动画事件只决定触发时刻，授权来自令牌</b>（review finding A）：必须先有一枚由
     * 权威 CASTING 武装、未过期、未被作废的 {@link #animToken}，且该 animId 正是这枚令牌所属
     * 招式的 charge / release 动画，才允许触发。charge 与 release 各自**一次性消费**
     * （{@code chargeFired} / {@code releaseFired}），release 消费后 charge 也不再触发（不回退）。
     * 于是：未 accepted 就来的 PlayAnim、重复 PlayAnim、reject/INTERRUPT/teardown 之后迟到的
     * PlayAnim，全部 no-op。
     *
     * <p>非本地玩家（别人放大招不震我的相机）/ 非登记动画 / 无令牌 = no-op。两段共享
     * {@link CameraShakeController} 单通道，release 的 SUSTAIN 顶替 charge 的 CRESCENDO。
     */
    public static void onAnimPlayed(UUID targetPlayer, Identifier animId) {
        if (targetPlayer == null || animId == null || !localPlayerPredicate.isLocal(targetPlayer)) {
            return;
        }
        long now = clock.getAsLong();
        // 两个通道在同一临界区落地，与 teardown / onJuiceMultiplierChanged 的清空对称——
        // 否则「清脉冲…清抖动」与「建脉冲…起抖动」交错时会漏下一条孤儿抖动。
        // 倍率也在锁内读，与 fire() 同：否则「读到非 0」与「落地」之间被归 0 插进来，
        // 会绕过取消路径留下僵尸抖动。
        synchronized (LOCK) {
            AnimCastToken token = liveAnimToken(now);
            if (token == null) {
                return;  // 无已 accepted 的在飞施法 → 动画事件不授权任何 juice
            }
            AnimDrivenJuice juice = token.juice;
            boolean isCharge = juice.chargeAnim().equals(animId);
            boolean isRelease = juice.releaseAnim().equals(animId);
            if (!isCharge && !isRelease) {
                return;  // 该动画不属于持令牌的这一招 → 不得挪用
            }
            if (token.releaseFired || (isCharge && token.chargeFired)) {
                return;  // 一次性消费：重复 charge / 重复 release / release 之后的迟到 charge
            }
            float scale = JuiceConfig.juiceMultiplier();
            if (scale <= 0f) {
                return;  // juice 全局关闭（不消费令牌：关闭期间的事件视为未发生）
            }
            if (isCharge) {
                token.chargeFired = true;
                triggerShake(juice.charge().peakIntensity() * scale,
                    juice.charge().buildDurationTicks(), juice.charge().envelope(), now);
                return;
            }
            token.releaseFired = true;
            // release 是这次施法的终点：落 FIRED 终态，之后同身份的权威 CASTING 也不再重开令牌。
            markTerminal(token.id, Terminal.FIRED);
            ReleaseBurst burst = juice.release();
            if (burst.fovPeakDegrees() > 0f && burst.fovDurationTicks() > 0) {
                pulse = new Pulse(burst.fovPeakDegrees() * scale, burst.fovDurationTicks(), now);
            }
            triggerShake(burst.shakeIntensity() * scale, burst.shakeDurationTicks(),
                burst.envelope(), now);
        }
    }

    /** 当前可用令牌（LOCK 内调用）：过期即清掉并返回 null。 */
    private static AnimCastToken liveAnimToken(long now) {
        AnimCastToken token = animToken;
        if (token == null) {
            return null;
        }
        if (now - token.armedAtMs > ANIM_TOKEN_TTL_MS || now < token.armedAtMs) {
            animToken = null;  // 过期（或时钟回跳）：不再授权
            return null;
        }
        return token;
    }

    /** 施法 juice 的抖动通道写入（LOCK 内调用；方向固定对角，见 {@link #CAST_SHAKE_DIR_X}）。 */
    private static void triggerShake(
        float intensity, int durationTicks, CameraShakeController.Envelope envelope, long now
    ) {
        CameraShakeController.triggerDirect(
            intensity, durationTicks, CAST_SHAKE_DIR_X, CAST_SHAKE_DIR_Z, false, envelope, now);
    }

    /**
     * 该招的动画事件驱动契约；未登记（或 {@code null}）返回 {@code null}。
     * {@code Map.of} 的 {@code get(null)} 会抛 NPE，故显式挡 null——与
     * {@link CastJuiceProfiles#get} 的 null-safe 口径一致。
     */
    private static AnimDrivenJuice animDrivenJuice(String skillId) {
        return skillId == null ? null : ANIM_DRIVEN_SKILLS.get(skillId);
    }

    /** heaven_gate 两段动画 juice 参数的只读查询 seam（测试逐字段 pin 用，不放宽封装）。 */
    static ChargeShake chargeAnimJuice(String skillId) {
        AnimDrivenJuice juice = animDrivenJuice(skillId);
        return juice == null ? null : juice.charge();
    }

    /** 同 {@link #chargeAnimJuice}：release 段参数只读查询。 */
    static ReleaseBurst releaseAnimJuice(String skillId) {
        AnimDrivenJuice juice = animDrivenJuice(skillId);
        return juice == null ? null : juice.release();
    }

    /** 动画事件驱动招式的 id 集合（测试 pin 用：登记集合必须精确相等）。 */
    static Set<String> animDrivenSkillIds() {
        return ANIM_DRIVEN_SKILLS.keySet();
    }

    /** 该招登记的 charge / release 动画 id（测试 pin：动画↔招式的关联契约）。 */
    static Identifier chargeAnimId(String skillId) {
        AnimDrivenJuice juice = animDrivenJuice(skillId);
        return juice == null ? null : juice.chargeAnim();
    }

    /** 同 {@link #chargeAnimId}：release 段动画 id。 */
    static Identifier releaseAnimId(String skillId) {
        AnimDrivenJuice juice = animDrivenJuice(skillId);
        return juice == null ? null : juice.releaseAnim();
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

    /**
     * 断线 / 切世界 / 死亡：立即复位基准 FOV + 清 pending + 清抖动（幂等，重复调用无副作用）。
     *
     * <p><b>在飞身份整批落 VOIDED 终态</b>：只清 pending 不够——teardown 之后一条迟到的
     * 同 identity CASTING 会重新武装、随后的 COMPLETE 就能在死亡/换世界之后放出 juice。
     * 把当时在飞的身份记进 {@link #terminals} 即封死这条路；全新施法有新的 startedAtMs，
     * 不受影响。
     *
     * <p>残留缺口（诚实标注）：若某次施法的权威 CASTING 在 teardown **之后**才首次到达，
     * 本地没有任何凭据把它和「旧会话」区分开（wire 无 cast id / session id），它会被当成
     * 新施法。死亡路径上这一帧的影响也被兜住：{@link #tick()} 在死亡界面可见期间**每 tick**
     * 都 teardown，故最多存活 ≤1 tick。
     */
    public static void teardown() {
        // pulse 清空与 fire() 写入同锁，避免死亡/断线 teardown 被随后落地的 fire 复活一帧。
        // 抖动与脉冲同处一个临界区：两个通道的写入都串行化在 LOCK 上，否则网络线程的 fire
        // 可能恰好落在「清脉冲」与「清抖动」之间，留下一条孤儿抖动。
        synchronized (LOCK) {
            pulse = null;
            if (pending != null) {
                markTerminal(pending.id(), Terminal.VOIDED);
                pending = null;
            }
            if (animToken != null) {
                markTerminal(animToken.id, Terminal.VOIDED);
                animToken = null;  // teardown 后迟到的 charge/release 动画不再授权
            }
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

    /**
     * slot → skillId。juice profile 皆技能栏技能；QUICK_SLOT = 快捷物品，无技能 profile。
     *
     * <p><b>本地预测在此处（且仅在此处）起作用</b>：{@code source} 不是 wire 字段，
     * {@code CastSyncHandler.sourceFor} 靠「当前快照是否正 CASTING 在同一 slot 且为 SKILL_BAR」
     * 推断它——而那条快照正是按键时的本地预测（{@code SkillBarKeyRouter.beginSkillBarCast}）。
     * 所以预测是权威 CASTING 能被认成技能栏施法的**前提**，但它本身不武装任何东西（门控见类
     * 文档）。预测已被本地打断/清掉时 {@code sourceFor} 退化成 QUICK_SLOT → 解析不出 profile
     * → 不武装；那种情形下这次施法本来就已被玩家取消，不触发 juice 是正确结果。
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

    /**
     * 某 cast identity 的终态（只读查询 seam，{@link Identity} 是私有 record 故按二元组收参）。
     * 测试用它 pin「触发过的记 FIRED、被打断的记 VOIDED」——两者都禁止再武装，但语义不同，
     * 混淆会让「已 fire 的身份被一条迟到打断改写」这类回归悄悄溜过。
     *
     * @return 该身份的终态，未终态返回 {@code null}
     */
    static Terminal terminalStateForTest(int slot, long startedAtMs) {
        synchronized (LOCK) {
            return terminals.get(new Identity(slot, startedAtMs));
        }
    }

    /** ENTITY_UNLOAD 本地玩家实体判定 seam（单测注入，免构造 ClientPlayerEntity）。 */
    static void setLocalPlayerEntityPredicateForTest(LocalPlayerEntityPredicate p) {
        localPlayerEntityPredicate = p == null ? (e -> e instanceof ClientPlayerEntity) : p;
    }

    /** public 理由同 {@link #setLocalPlayerPredicateForTest}（跨包路由接线测试要复位 juice 状态）。 */
    public static void resetForTests() {
        synchronized (LOCK) {
            pulse = null;
            pending = null;
            animToken = null;
            terminals.clear();
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

    /**
     * 已由权威 CASTING 武装、等待 release 的 juice。<b>没有 {@code fired} 标记</b>——触发即被
     * 消费掉（置 null）并在 {@link #terminals} 落 FIRED，幂等由那份跨 pending 生命周期的终态
     * 记录保证；标记留在 pending 里会随 supersession/清场一起丢失（review finding C）。
     */
    private record Pending(Identity id, CastJuiceProfile profile) {
    }
}
