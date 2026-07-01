package com.bong.client.network;

import com.bong.client.animation.AnimationLayerManager;
import com.bong.client.animation.BongAnimationPlayer;
import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.cultivation.BreakthroughCinematicPayload;
import com.bong.client.cultivation.BreakthroughRenderState;
import com.bong.client.cultivation.BreakthroughRenderStateStore;
import com.bong.client.cultivation.BreakthroughSpectacleRenderer;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.particle.BongVfxParticleBridge;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.LongSupplier;

/**
 * 突破演出 payload handler。
 *
 * <p>除了既有的 toast + {@link VisualEffectState}（屏幕特效）之外，还要把
 * {@link BreakthroughSpectacleRenderer.SpectaclePlan} 里的 vfxEventIds / audioRecipeId /
 * animationId 真正接到对应引擎，否则突破阶段就只剩屏幕特效，粒子/音效/动作全部沉默。
 *
 * <p>接线方式镜像 {@link ShieldBrokenHandler}：
 * <ul>
 *   <li>粒子：{@link VfxParticleBridge#spawnParticle} 用 payload 携带的 world_pos 作为
 *       origin（不像破盾只发生在本地玩家身上，突破可能是任意 actor，payload 坐标才准确）</li>
 *   <li>音效：内联构建 {@link AudioRecipe}（惯用法同 {@link ShieldBrokenHandler} /
 *       {@code NicheIntrusionAlertHandler}），按 audioRecipeId 差异化音色，
 *       经 {@link AudioPlaybackBridge#play} 播放</li>
 *   <li>动作：{@link AnimationLayerManager#play} 走 {@code FULL_BODY} 语义层
 *       （打坐/突破属于全身动作），目标是本地玩家——toast 文案本身就是第一人称叙事
 *       （"灵气开始聚拢"），说明这条消息语义上就是"你自己"的突破演出</li>
 * </ul>
 *
 * <p>测试注入点同 {@link ShieldBrokenHandler}：{@link #setParticleBridgeForTests} /
 * {@link #setAudioBridgeForTests}。动画调用在无 {@link MinecraftClient} 的单测环境下
 * 因 {@code MinecraftClient.getInstance()==null} 自然短路，不需要单独注入。
 */
public final class BreakthroughCinematicHandler implements ServerDataHandler {

    private static final int BREAKTHROUGH_AUDIO_PRIORITY = 75;
    private static final AtomicLong AUDIO_INSTANCE_SEQ = new AtomicLong(95_000L);

    private final LongSupplier nowMillisSupplier;

    /** 粒子 bridge 注入点，测试时替换为 stub（默认 null → 使用 BongVfxParticleBridge 单例）。 */
    private static VfxParticleBridge particleBridgeOverride = null;

    /** 音效 bridge 注入点，测试时替换为 stub（默认 null → 使用全局 SoundRecipePlayer）。 */
    private static AudioPlaybackBridge audioBridgeOverride = null;

    public BreakthroughCinematicHandler() {
        this(System::currentTimeMillis);
    }

    BreakthroughCinematicHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    static void setParticleBridgeForTests(VfxParticleBridge bridge) {
        particleBridgeOverride = bridge;
    }

    static void resetParticleBridgeForTests() {
        particleBridgeOverride = null;
    }

    static void setAudioBridgeForTests(AudioPlaybackBridge bridge) {
        audioBridgeOverride = bridge;
    }

    static void resetAudioBridgeForTests() {
        audioBridgeOverride = null;
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        BreakthroughCinematicPayload payload = BreakthroughCinematicPayload.parse(envelope.payload());
        if (payload == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring breakthrough_cinematic payload: required fields missing or invalid"
            );
        }

        long nowMillis = nowMillisSupplier.getAsLong();
        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonStateStore.snapshot(), nowMillis);
        VisualEffectState visualEffectState = VisualEffectState.create(
            plan.visualEffectType(),
            plan.visualIntensity(),
            plan.visualDurationMillis(),
            nowMillis
        );
        ServerDataDispatch.ToastSpec toastSpec =
            new ServerDataDispatch.ToastSpec(plan.toastText(), plan.toastColor(), 2_400L);

        triggerParticles(plan, payload);
        triggerAudio(plan);
        triggerAnimation(plan);

        // F7：缓存渲染状态供 BreakthroughBillboardWorldRenderer 逐帧读取（远景方位指示器）。
        BreakthroughRenderStateStore.replace(
            new BreakthroughRenderState(payload, nowMillis + plan.visualDurationMillis())
        );

        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            toastSpec,
            visualEffectState,
            "breakthrough_cinematic accepted phase=" + payload.phase().wireName()
                + " realm=" + payload.realmFrom() + "->" + payload.realmTo()
                + " vfx=" + String.join(",", plan.vfxEventIds())
                + " audio=" + plan.audioRecipeId()
                + " anim=" + plan.animationId()
        );
    }

    /**
     * 按 vfxEventIds 逐个派发粒子，origin 用 payload 携带的世界坐标（突破发生地点）。
     * 未知/空列表安全跳过，不影响 toast/flash 主流程。
     */
    private static void triggerParticles(
        BreakthroughSpectacleRenderer.SpectaclePlan plan,
        BreakthroughCinematicPayload payload
    ) {
        List<String> vfxEventIds = plan.vfxEventIds();
        if (vfxEventIds.isEmpty()) {
            return;
        }
        VfxParticleBridge particles = particleBridgeOverride != null
            ? particleBridgeOverride : new BongVfxParticleBridge();
        double[] origin = new double[]{payload.worldX(), payload.worldY(), payload.worldZ()};
        for (String vfxId : vfxEventIds) {
            Identifier eventId = Identifier.tryParse(vfxId);
            if (eventId == null) {
                continue;
            }
            particles.spawnParticle(new VfxEventPayload.SpawnParticle(
                eventId,
                origin,
                Optional.empty(),
                OptionalInt.empty(),
                Optional.of(plan.visualIntensity()),
                OptionalInt.empty(),
                OptionalInt.empty()
            ));
        }
    }

    /** 按 audioRecipeId 内联构建 AudioRecipe 并播放；空 id 安全跳过。 */
    private static void triggerAudio(BreakthroughSpectacleRenderer.SpectaclePlan plan) {
        String recipeId = plan.audioRecipeId();
        if (recipeId == null || recipeId.isBlank()) {
            return;
        }
        AudioPlaybackBridge audio = audioBridgeOverride != null
            ? audioBridgeOverride : SoundRecipePlayer.instance();
        audio.play(new AudioEventPayload.PlaySoundRecipe(
            recipeId,
            AUDIO_INSTANCE_SEQ.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            audioRecipeFor(recipeId)
        ));
    }

    /** 全身动作层播放；无本地玩家（单测/未就绪）时安全跳过。 */
    private static void triggerAnimation(BreakthroughSpectacleRenderer.SpectaclePlan plan) {
        String animId = plan.animationId();
        if (animId == null || animId.isBlank()) {
            return;
        }
        AbstractClientPlayerEntity player = localPlayer();
        if (player == null) {
            return;
        }
        AnimationLayerManager.play(
            player,
            new Identifier("bong", animId),
            AnimationLayerManager.Channel.FULL_BODY.priority(),
            BongAnimationPlayer.DEFAULT_FADE_IN_TICKS
        );
    }

    private static AbstractClientPlayerEntity localPlayer() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc == null) {
            return null;
        }
        return mc.player;
    }

    /**
     * 按阶段音色差异化 AudioRecipe（各阶段应有意义地对应，非单一音效复用）：
     * prelude 慢心跳 → charge 快心跳 → catalyze 共鸣嗡鸣 → apex 钟鸣 → aftermath
     * 按结果分叉为打断/失败的挫音 或 成功的余韵。未知 id 走保守 fallback。
     */
    private static AudioRecipe audioRecipeFor(String recipeId) {
        List<AudioLayer> layers = switch (recipeId) {
            case "breakthrough_heartbeat_slow" -> List.of(
                new AudioLayer(new Identifier("minecraft", "block.beacon.ambient"), 0.4f, 0.7f, 0)
            );
            case "breakthrough_heartbeat_fast" -> List.of(
                new AudioLayer(new Identifier("minecraft", "block.beacon.ambient"), 0.6f, 1.3f, 0),
                new AudioLayer(new Identifier("minecraft", "block.conduit.ambient"), 0.3f, 1.1f, 2)
            );
            case "breakthrough_resonance_hum" -> List.of(
                new AudioLayer(new Identifier("minecraft", "block.conduit.activate"), 0.6f, 0.9f, 0)
            );
            case "breakthrough_bell" -> List.of(
                new AudioLayer(new Identifier("minecraft", "block.bell.use"), 0.8f, 1.0f, 0)
            );
            case "breakthrough_interrupted" -> List.of(
                new AudioLayer(new Identifier("minecraft", "entity.villager.no"), 0.7f, 0.8f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.item.break"), 0.6f, 1.0f, 2)
            );
            case "breakthrough_failure" -> List.of(
                new AudioLayer(new Identifier("minecraft", "entity.wither.hurt"), 0.6f, 0.7f, 0)
            );
            case "breakthrough_afterglow" -> List.of(
                new AudioLayer(new Identifier("minecraft", "entity.experience_orb.pickup"), 0.5f, 0.9f, 0)
            );
            default -> List.of(
                new AudioLayer(new Identifier("minecraft", "block.beacon.ambient"), 0.4f, 1.0f, 0)
            );
        };
        return new AudioRecipe(
            recipeId,
            layers,
            Optional.empty(),
            BREAKTHROUGH_AUDIO_PRIORITY,
            AudioAttenuation.WORLD_3D,
            AudioCategory.PLAYERS,
            AudioBus.ENVIRONMENT
        );
    }
}
