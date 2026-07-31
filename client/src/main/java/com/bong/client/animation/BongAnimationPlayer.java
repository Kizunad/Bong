package com.bong.client.animation;

import dev.kosmx.playerAnim.api.firstPerson.FirstPersonConfiguration;
import dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;
import dev.kosmx.playerAnim.api.layered.ModifierLayer;
import dev.kosmx.playerAnim.api.layered.modifier.AbstractFadeModifier;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import dev.kosmx.playerAnim.core.util.Ease;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * 播放抽象：把"在玩家上播 id 这个动画"封装成两行 API。
 *
 * <p>实现要点：
 * <ul>
 *   <li>每个 (player, animId) 对对应一个 {@link ModifierLayer}，存在 {@link #ACTIVE_LAYERS} 里，
 *       方便 {@link #stop} 精准移除。</li>
 *   <li>再次 {@link #play} 同 id 时走 {@code replaceAnimationWithFade}——同层上替换动画，
 *       享受 PlayerAnimator 自带的淡入淡出。</li>
 *   <li>首次播放用 {@code AnimationStack.addAnimLayer} 插入新层；
 *       fade-in 靠 {@code standardFadeIn} modifier 实现。</li>
 *   <li>priority 按 plan-player-animation-v1 §3.3 划档：
 *       100-499 姿态 / 500-999 移动 / 1000-1999 战斗 / 2000-2999 受击 / 3000+ 剧情。</li>
 *   <li>{@link #stop} 后不立刻 {@link AnimationStack#removeLayer}，而是推到
 *       {@link #PENDING_REMOVALS} 队列里等 fade-out 完成后再清——在 PlayerAnimator
 *       仍持有引用时 removeLayer 会制造 inactive modifier 状态跳变，先让 fade
 *       自然跑完再摘层最稳妥。</li>
 * </ul>
 *
 * <p>Phase 1 假设只播本地玩家，多人广播 Phase 2 再接。
 */
public final class BongAnimationPlayer {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-anim-player");

    /** 默认淡入 tick 数——大多数战斗动画 3 tick (0.15s) 够快。 */
    public static final int DEFAULT_FADE_IN_TICKS = 3;
    /** 默认淡出 tick 数——稍长一点让停止显得柔和。 */
    public static final int DEFAULT_FADE_OUT_TICKS = 5;
    /** fade 完成后再额外等 1 tick 才把层从 stack 里摘——保险余量，
     *  避免 fade 最后一帧和 removeLayer 撞在同一 tick 上。 */
    private static final int REMOVAL_SAFETY_MARGIN_TICKS = 1;

    /** 嵌套 Map：玩家 UUID → (动画 id → 其原始 AnimationStack 上的当前激活层)。 */
    private static final Map<UUID, Map<Identifier, ActiveLayer>> ACTIVE_LAYERS =
        new HashMap<>();

    /**
     * 待 fade-out 结束后从 AnimationStack 中摘除的层队列。
     * {@link #tickPendingRemovals} 每 tick 扣 1，到 0 调 {@link AnimationStack#removeLayer}。
     *
     * <p>持有 {@link AnimationStack} 的直接引用而不是 player，原因是：player entity
     * 在世界卸载 / 重连时会被 GC，但 AnimationStack 的生命周期由 PlayerAnimator 的
     * mixin 管——我们只持有"存活的 stack 引用"，player 消失后 stack 也跟着 GC，
     * 我们的 Pending 会变成悬空引用，但 {@link #tickPendingRemovals} 用 try/catch
     * 包住 removeLayer，悬空时静默回收，不 crash。
     */
    private static final List<PendingRemoval> PENDING_REMOVALS =
        Collections.synchronizedList(new ArrayList<>());

    /** bootstrap 标记，防止 init 重复注册 tick 钩子——多次 init 会累加 tick 回调。 */
    private static boolean initialized = false;

    private BongAnimationPlayer() {
    }

    /** 客户端启动时调一次：挂一个 {@link ClientTickEvents#END_CLIENT_TICK} 消费
     *  {@link #PENDING_REMOVALS} 队列，把 fade-out 结束的层从 AnimationStack 里摘掉。 */
    public static void init() {
        if (initialized) {
            return;
        }
        initialized = true;
        ClientTickEvents.END_CLIENT_TICK.register(client -> tickPendingRemovals());
    }

    /** 便捷重载：默认 fade-in。 */
    public static boolean play(AbstractClientPlayerEntity player, Identifier animId, int priority) {
        return play(player, animId, priority, DEFAULT_FADE_IN_TICKS);
    }

    /**
     * 在玩家上播 animId 对应的动画。
     *
     * @return true=开始播，false=animId 未注册或 player 不可动画化
     */
    public static boolean play(
        AbstractClientPlayerEntity player,
        Identifier animId,
        int priority,
        int fadeInTicks
    ) {
        if (player == null || animId == null) {
            return false;
        }
        return playOnStack(
            PlayerAnimationAccess.getPlayerAnimLayer(player),
            player.getUuid(),
            animId,
            priority,
            fadeInTicks
        );
    }

    /**
     * 播放的可测试 seam：接收 {@link AnimationStack} + 玩家 UUID，省去对 {@code AbstractClientPlayerEntity}
     * 的依赖——单测里 mixin 接入的 {@code IPlayer} 接口无法用，必须有这层 split。
     * 生产路径 {@link #play} 直接委托到这里。
     */
    static boolean playOnStack(
        AnimationStack stack,
        UUID pid,
        Identifier animId,
        int priority,
        int fadeInTicks
    ) {
        if (stack == null || pid == null || animId == null) {
            return false;
        }
        FpvResolution resolution = resolveFpvContent(pid, animId);

        KeyframeAnimation anim = BongAnimationRegistry.get(resolution.contentId());
        if (anim == null) {
            return false;
        }

        KeyframeAnimationPlayer framePlayer = new KeyframeAnimationPlayer(anim);
        applyFirstPersonRendering(framePlayer, resolution.useFpvArms());

        Map<Identifier, ActiveLayer> perPlayer =
            ACTIVE_LAYERS.computeIfAbsent(pid, k -> new HashMap<>());
        ActiveLayer existing = perPlayer.get(animId);
        if (existing != null) {
            // 同 id 重触发：在现有层上淡入替换，连击时这条路径保证平滑过渡——
            // 不新增 AnimationStack 条目，避免同 animId 叠 N 层。
            existing.layer.replaceAnimationWithFade(
                AbstractFadeModifier.standardFadeIn(Math.max(0, fadeInTicks), Ease.INOUTSINE),
                framePlayer
            );
            return true;
        }

        // 首次：新建层，插进玩家的 AnimationStack，并挂上 fade-in modifier
        ModifierLayer<KeyframeAnimationPlayer> layer = new ModifierLayer<>(framePlayer);
        if (fadeInTicks > 0) {
            layer.addModifierLast(AbstractFadeModifier.standardFadeIn(fadeInTicks, Ease.INOUTSINE));
        }
        stack.addAnimLayer(priority, layer);
        perPlayer.put(animId, new ActiveLayer(stack, layer));
        return true;
    }

    /**
     * 第一人称渲染配置（plan-fpv-cast-av-v1 路线 A，§8.1 #1 真机拍板）。
     *
     * <p>始终用 {@link FirstPersonMode#THIRD_PERSON_MODEL}：库 {@code ItemInHandRendererMixin}
     * 在此模式下整段 cancel vanilla 第一人称手/物渲染，改由模型渲染、受 {@link FirstPersonConfiguration}
     * 门控（player-anim 1.0.2-rc1 无 {@code ENABLED} 值，此为库原生第一人称正路）。库默认 config 的
     * {@code showRightArm/showLeftArm=false} 会隐藏手臂——这正是出厂第一人称只见持物无手臂的根因。
     *
     * @param useFpvArms true（本地玩家 + 命中 {@code _fpv} 变体）→ 开双臂 + 双手持物；
     *                   false（无变体 / 远端玩家）→ 库默认 config（隐藏手臂，出厂行为不变）
     */
    static void applyFirstPersonRendering(KeyframeAnimationPlayer framePlayer, boolean useFpvArms) {
        if (useFpvArms) {
            framePlayer
                .setFirstPersonConfiguration(
                    new FirstPersonConfiguration()
                        .setShowRightArm(true)
                        .setShowLeftArm(true)
                        .setShowRightItem(true)
                        .setShowLeftItem(true))
                .setFirstPersonMode(FirstPersonMode.THIRD_PERSON_MODEL);
        } else {
            framePlayer.setFirstPersonMode(FirstPersonMode.THIRD_PERSON_MODEL);
        }
    }

    /**
     * {@link #resolveFpvContent} 结果：要播的动画 id（原招或 {@code _fpv} 变体）+ 是否开
     * 第一人称双臂。作为 {@link #playOnStack} 内 FPV 分支逻辑的可测契约（plan §P1）。
     */
    record FpvResolution(Identifier contentId, boolean useFpvArms) {
    }

    /**
     * FPV 内容解析（plan-fpv-cast-av-v1 P1，纯逻辑可测 seam）：本地玩家播 {@code <id>} 时
     * 优先取 {@code <id>_fpv} 变体（贴脸视角专调姿态，见 docs/player-animation-conventions.md §16）
     * —— 命中则 {@code (变体 id, useFpvArms=true)}（路线 A，§8.1 #1：THIRD_PERSON_MODEL +
     * FirstPersonConfiguration 开双臂/持物）；无变体或远端玩家 → {@code (原 id, false)}（TPV 动画
     * + 库默认 config，第一人称隐藏手臂，出厂行为）。**远端玩家渲染分支零变化**（FPV 只影响本地
     * 玩家，plan §P1 硬约束）。
     *
     * <p>本地玩家判定经 {@link #localPlayerPredicate} seam 注入——生产 = MinecraftClient 当前玩家，
     * 单测经 {@link #setLocalPlayerPredicateForTest} 驱动本分支（headless 下
     * {@code MinecraftClient.getInstance()==null}，否则此 wiring 永不被自动化测试覆盖）。
     */
    static FpvResolution resolveFpvContent(UUID pid, Identifier animId) {
        if (localPlayerPredicate.isLocal(pid)) {
            Identifier fpvId = fpvVariantId(animId);
            if (BongAnimationRegistry.contains(fpvId)) {
                return new FpvResolution(fpvId, true);
            }
        }
        return new FpvResolution(animId, false);
    }

    /** 本地玩家判定 seam（{@link #resolveFpvContent}）。生产默认 = MinecraftClient 当前玩家。 */
    @FunctionalInterface
    interface LocalPlayerPredicate {
        boolean isLocal(UUID pid);
    }

    /**
     * 生产实现：仅真实客户端上、UUID == 当前本地玩家时为 true。单测 / 无 client 环境
     * （{@code getInstance()==null} 或无 player）返回 false——故 {@code _fpv} 变体路径默认只在
     * 真实客户端本地玩家上触发，远端玩家路径与既有 playOnStack 单测均不受影响。
     */
    private static boolean isLocalPlayerFromClient(UUID pid) {
        MinecraftClient mc = MinecraftClient.getInstance();
        return mc != null && mc.player != null && mc.player.getUuid().equals(pid);
    }

    /** 当前生效的本地玩家判定（默认生产实现，单测可注入）。 */
    private static LocalPlayerPredicate localPlayerPredicate =
        BongAnimationPlayer::isLocalPlayerFromClient;

    /**
     * 测试钩子：注入本地玩家判定，驱动 {@link #resolveFpvContent} 的 FPV 变体分支
     * （headless 下真实 MinecraftClient 恒 null，该分支否则不可自动化回归）。用后必须
     * {@link #resetForTest} 复位，防跨测试污染。
     */
    static void setLocalPlayerPredicateForTest(LocalPlayerPredicate predicate) {
        localPlayerPredicate = predicate;
    }

    /** {@code <ns>:<path>} → {@code <ns>:<path>_fpv}（本地玩家第一人称变体查找 id，plan §P1）。 */
    static Identifier fpvVariantId(Identifier animId) {
        return new Identifier(animId.getNamespace(), animId.getPath() + "_fpv");
    }

    /** 便捷重载：默认 fade-out。 */
    public static boolean stop(AbstractClientPlayerEntity player, Identifier animId) {
        return stop(player, animId, DEFAULT_FADE_OUT_TICKS);
    }

    /**
     * 停止并移除指定 id 的动画。fadeOutTicks &gt; 0 时先淡出再移除；
     * fadeOutTicks == 0 时立即从 AnimationStack 中摘除。
     *
     * @return true=找到并处理，false=该玩家/id 当前没在播
     */
    public static boolean stop(AbstractClientPlayerEntity player, Identifier animId, int fadeOutTicks) {
        if (player == null || animId == null) {
            return false;
        }
        return stopOnStack(
            PlayerAnimationAccess.getPlayerAnimLayer(player),
            player.getUuid(),
            animId,
            fadeOutTicks
        );
    }

    /** 停止的可测试 seam，见 {@link #playOnStack}。 */
    static boolean stopOnStack(AnimationStack stack, UUID pid, Identifier animId, int fadeOutTicks) {
        if (stack == null || pid == null || animId == null) {
            return false;
        }
        Map<Identifier, ActiveLayer> perPlayer = ACTIVE_LAYERS.get(pid);
        if (perPlayer == null) {
            return false;
        }
        ActiveLayer active = perPlayer.remove(animId);
        if (active == null) {
            return false;
        }
        if (perPlayer.isEmpty()) {
            ACTIVE_LAYERS.remove(pid);
        }
        if (fadeOutTicks > 0) {
            // fade 到 null 等价于淡出到默认姿态
            active.layer.replaceAnimationWithFade(
                AbstractFadeModifier.standardFadeIn(fadeOutTicks, Ease.INOUTSINE),
                null
            );
            // fade 完成后再摘层，避免 fade 过程中 AnimationStack 突然没了这条
            // 引用导致渲染瞬间跳帧。层必须从自己的原始 stack 移除。
            PENDING_REMOVALS.add(new PendingRemoval(
                active.stack, active.layer, fadeOutTicks + REMOVAL_SAFETY_MARGIN_TICKS
            ));
        } else {
            // 无淡出：立刻从层自己的原始 stack 摘除，保持 AnimationStack 清洁。
            removeLayer(active.stack, active.layer, "立即");
        }
        return true;
    }

    /**
     * 断线时终结旧会话的 PlayerAnimator 绑定。
     *
     * <p>静态缓存同时持有旧 player 的 {@link AnimationStack} 和延迟淡出层；只清 Map/List 会让
     * 旧 stack 继续持有层，且 pending closure 保留到下一次 tick。断线时不需要淡出，因此逐一从
     * 各自的原始 stack 立即摘除，再清空会话数据。{@link #initialized}、tick hook 与本地玩家
     * 判定 seam 都是客户端进程级 wiring，必须保留。
     */
    public static void clearOnDisconnect() {
        synchronized (ACTIVE_LAYERS) {
            for (Map<Identifier, ActiveLayer> perPlayer : ACTIVE_LAYERS.values()) {
                for (ActiveLayer active : perPlayer.values()) {
                    removeLayer(active.stack, active.layer, "断线 active");
                }
            }
            ACTIVE_LAYERS.clear();
        }
        synchronized (PENDING_REMOVALS) {
            for (PendingRemoval pending : PENDING_REMOVALS) {
                removeLayer(pending.stack, pending.layer, "断线 pending");
            }
            PENDING_REMOVALS.clear();
        }
    }

    /** 测试/诊断用：玩家当前正在播的动画 id 集合。 */
    public static java.util.Set<Identifier> activeAnimations(UUID playerId) {
        Map<Identifier, ActiveLayer> perPlayer = ACTIVE_LAYERS.get(playerId);
        return perPlayer == null ? java.util.Set.of() : java.util.Set.copyOf(perPlayer.keySet());
    }

    /** 测试/诊断用：当前待 AnimationStack.removeLayer 的层数。泄漏回归的直接信号。 */
    public static int pendingRemovalsSize() {
        synchronized (PENDING_REMOVALS) {
            return PENDING_REMOVALS.size();
        }
    }

    /** 测试钩子：直接跑一次队列扣减逻辑，无需挂 ClientTickEvents。 */
    static void tickPendingRemovalsForTest() {
        tickPendingRemovals();
    }

    /** 测试钩子：绕过 stop() 直接塞 pending entry——stop 依赖 player entity，
     *  单测里没 MinecraftClient，所以 pending 队列本身的逻辑需要从外部构造状态来验证。 */
    static void schedulePendingRemovalForTest(
        AnimationStack stack,
        ModifierLayer<KeyframeAnimationPlayer> layer,
        int ticks
    ) {
        PENDING_REMOVALS.add(new PendingRemoval(stack, layer, ticks));
    }

    /** 测试钩子：清零 pending 队列 + 复位本地玩家判定 seam，防止跨测试污染。 */
    static void resetForTest() {
        synchronized (PENDING_REMOVALS) {
            PENDING_REMOVALS.clear();
        }
        synchronized (ACTIVE_LAYERS) {
            ACTIVE_LAYERS.clear();
        }
        localPlayerPredicate = BongAnimationPlayer::isLocalPlayerFromClient;
    }

    /** 每 client tick 扣 1；到 0 的从 AnimationStack 摘除并出队。 */
    private static void tickPendingRemovals() {
        synchronized (PENDING_REMOVALS) {
            if (PENDING_REMOVALS.isEmpty()) {
                return;
            }
            Iterator<PendingRemoval> it = PENDING_REMOVALS.iterator();
            while (it.hasNext()) {
                PendingRemoval p = it.next();
                p.remainingTicks--;
                if (p.remainingTicks <= 0) {
                    it.remove();
                    removeLayer(p.stack, p.layer, "延迟");
                }
            }
        }
    }

    /** 统一容错摘层：world 卸载期间旧 stack 可能已失效，清理不得导致客户端崩溃。 */
    private static void removeLayer(
        AnimationStack stack,
        ModifierLayer<KeyframeAnimationPlayer> layer,
        String context
    ) {
        try {
            stack.removeLayer(layer);
        } catch (RuntimeException ex) {
            LOGGER.warn("[bong/anim] {} removeLayer 抛错（可忽略）: {}", context, ex.toString());
        }
    }

    /** 活跃动画层及其所属 stack，供断线时从原始 stack 物理摘除。 */
    private static final class ActiveLayer {
        final AnimationStack stack;
        final ModifierLayer<KeyframeAnimationPlayer> layer;

        ActiveLayer(AnimationStack stack, ModifierLayer<KeyframeAnimationPlayer> layer) {
            this.stack = stack;
            this.layer = layer;
        }
    }

    /** 待清理的 (AnimationStack, ModifierLayer, 剩余 tick) 三元组。 */
    private static final class PendingRemoval {
        final AnimationStack stack;
        final ModifierLayer<KeyframeAnimationPlayer> layer;
        int remainingTicks;

        PendingRemoval(AnimationStack stack, ModifierLayer<KeyframeAnimationPlayer> layer, int ticks) {
            this.stack = stack;
            this.layer = layer;
            this.remainingTicks = ticks;
        }
    }
}
