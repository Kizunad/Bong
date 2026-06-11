package com.bong.client.network;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.particle.BongVfxParticleBridge;
import com.bong.client.visual.particle.ShieldWoodShatterPlayer;
import com.bong.client.BongClient;
import com.google.gson.JsonObject;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.concurrent.atomic.AtomicLong;

/**
 * plan-shield-block-v1 §P3：{@code shield_broken} payload 客户端 handler。
 *
 * <p>镜像 {@link WeaponBrokenHandler}，但语义独立：盾不是武器。
 * <ul>
 *   <li>toast 文案「盾已碎裂」，颜色 {@link #BROKEN_TOAST_COLOR}，时长同武器破损 toast</li>
 *   <li>VisualEffect {@code shield_break_flash}，强度 1.0，时长 260ms</li>
 *   <li>按 template_id 触发对应材质粒子 + 音效：
 *     <ul>
 *       <li>{@code wooden_shield} → 木屑粒子（{@code bong:shield_break_wood}）+ 木破音 recipe</li>
 *       <li>{@code bone_shield} → 骨白粒子（{@code bong:shield_break_bone}）+ 骨破音（在线 spawn）</li>
 *     </ul>
 *   </li>
 *   <li>破盾后调用 {@link EquippedShieldStore#clearIfInstance(long)} 清空 off_hand 盾槽</li>
 * </ul>
 */
public final class ShieldBrokenHandler implements ServerDataHandler {

    static final int BROKEN_TOAST_COLOR = 0xFFC04040;
    static final long BROKEN_TOAST_DURATION_MS = WeaponBrokenHandler.BROKEN_TOAST_DURATION_MS;
    static final long BROKEN_FLASH_DURATION_MS = WeaponBrokenHandler.BROKEN_FLASH_DURATION_MS;
    static final double BROKEN_FLASH_INTENSITY = 1.0;

    /**
     * 破盾粒子 event id：与 VfxBootstrap 注册键一致。
     * 骨盾复用 FaunaBoneShatterPlayer 但以盾专用 key 注册（非 fauna_bone_shatter），
     * 避免误触发其他场景的骨骼爆碎效果。
     */
    static final Identifier PARTICLE_EVENT_WOOD = ShieldWoodShatterPlayer.EVENT_ID; // bong:shield_break_wood
    static final Identifier PARTICLE_EVENT_BONE = new Identifier("bong", "shield_break_bone");

    private static final int SHIELD_BREAK_AUDIO_PRIORITY = 70;
    private static final AtomicLong AUDIO_INSTANCE_SEQ = new AtomicLong(90_000L);

    /** 注入点，测试时替换为 stub（默认 null → 使用全局 SoundRecipePlayer）。 */
    private static AudioPlaybackBridge audioBridgeOverride = null;

    /** 粒子 bridge 注入点，测试时替换为 stub（默认 null → 使用 BongVfxParticleBridge 单例）。 */
    private static VfxParticleBridge particleBridgeOverride = null;

    static void setAudioBridgeForTests(AudioPlaybackBridge bridge) {
        audioBridgeOverride = bridge;
    }

    static void resetAudioBridgeForTests() {
        audioBridgeOverride = null;
    }

    static void setParticleBridgeForTests(VfxParticleBridge bridge) {
        particleBridgeOverride = bridge;
    }

    static void resetParticleBridgeForTests() {
        particleBridgeOverride = null;
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        long instanceId = payload.has("instance_id")
            ? payload.get("instance_id").getAsLong() : 0L;
        String templateId = payload.has("template_id")
            ? payload.get("template_id").getAsString() : "unknown";

        BongClient.LOGGER.info(
            "[bong][shield] shield broken: instance={} template={}",
            instanceId, templateId
        );

        // 清空 off_hand 盾槽（破盾则盾消失）
        EquippedShieldStore.clearIfInstance(instanceId);

        // 按 template_id 差异化效果
        triggerMaterialEffects(templateId);

        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            new ServerDataDispatch.ToastSpec(
                "盾已碎裂",
                BROKEN_TOAST_COLOR,
                BROKEN_TOAST_DURATION_MS
            ),
            VisualEffectState.create(
                "shield_break_flash",
                BROKEN_FLASH_INTENSITY,
                BROKEN_FLASH_DURATION_MS,
                System.currentTimeMillis()
            ),
            "Shield broken: " + templateId + " (instance " + instanceId + ")"
        );
    }

    /**
     * 按 template_id 触发对应材质的粒子事件 + 音效。
     *
     * <p>粒子直接通过 {@link VfxParticleBridge#spawnParticle} 派发，
     * 使用本地玩家当前位置（无需服务器坐标）：破盾必然发生在本地玩家自己身上。
     * production 走 {@link BongVfxParticleBridge}，测试用 {@link #setParticleBridgeForTests}
     * 注入 stub 断言是否调用。
     *
     * <p>音效直接构建 AudioRecipe 内联播放，与 NicheIntrusionAlertHandler 惯用法一致。
     */
    private void triggerMaterialEffects(String templateId) {
        AudioPlaybackBridge audio = audioBridgeOverride != null
            ? audioBridgeOverride : SoundRecipePlayer.instance();
        VfxParticleBridge particles = particleBridgeOverride != null
            ? particleBridgeOverride : new BongVfxParticleBridge();

        if ("wooden_shield".equals(templateId)) {
            audio.play(woodBreakRecipePayload());
            spawnShieldParticle(particles, PARTICLE_EVENT_WOOD);
        } else if ("bone_shield".equals(templateId)) {
            audio.play(boneBreakRecipePayload());
            spawnShieldParticle(particles, PARTICLE_EVENT_BONE);
        } else {
            // 未知材质 fallback：播木破音 + 木屑粒子（保守降级）
            audio.play(woodBreakRecipePayload());
            spawnShieldParticle(particles, PARTICLE_EVENT_WOOD);
        }
    }

    /**
     * 在本地玩家位置触发破盾粒子。
     *
     * <p>破盾始终发生在当前玩家自己身上，所以直接取本地玩家坐标即可。
     * 若 MinecraftClient 未就绪（单测环境），bridge 的 spawnParticle 收到 null client 会
     * 返回 false，不影响流程。
     */
    private static void spawnShieldParticle(VfxParticleBridge bridge, Identifier eventId) {
        // 取本地玩家位置（主手位置 + 0.5 高度偏移，与盾举起位置相近）
        double[] origin = localPlayerOrigin();
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            eventId,
            origin,
            Optional.empty(),       // no direction override
            OptionalInt.empty(),    // use player default color
            Optional.empty(),       // default strength
            OptionalInt.empty(),    // default count
            OptionalInt.empty()     // default duration
        );
        bridge.spawnParticle(payload);
    }

    /**
     * 获取本地玩家位置（用于粒子原点）。
     * 若 client/player 未就绪则返回 [0,0,0]（不影响 bridge 的 false 返回）。
     */
    private static double[] localPlayerOrigin() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc == null) {
            return new double[]{0.0, 0.0, 0.0};
        }
        PlayerEntity player = mc.player;
        if (player == null) {
            return new double[]{0.0, 0.0, 0.0};
        }
        Vec3d pos = player.getPos();
        // 偏移到胸部高度，更接近盾牌举起的视觉位置
        return new double[]{pos.x, pos.y + 0.9, pos.z};
    }

    // ---- 音效内联 recipe -----------------------------------------------

    private static AudioEventPayload.PlaySoundRecipe woodBreakRecipePayload() {
        AudioRecipe recipe = new AudioRecipe(
            "shield_break_wood",
            List.of(
                new AudioLayer(new Identifier("minecraft", "entity.zombie.break_wooden_door"), 0.8f, 1.2f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.item.break"), 0.7f, 1.0f, 2)
            ),
            Optional.empty(),
            SHIELD_BREAK_AUDIO_PRIORITY,
            AudioAttenuation.WORLD_3D,
            AudioCategory.PLAYERS,
            AudioBus.COMBAT
        );
        return new AudioEventPayload.PlaySoundRecipe(
            "shield_break_wood",
            AUDIO_INSTANCE_SEQ.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        );
    }

    private static AudioEventPayload.PlaySoundRecipe boneBreakRecipePayload() {
        // 骨盾破碎：骨感 + 脆碎，差异化于木盾
        AudioRecipe recipe = new AudioRecipe(
            "shield_break_bone",
            List.of(
                new AudioLayer(new Identifier("minecraft", "entity.skeleton.hurt"), 0.7f, 0.9f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.item.break"), 0.6f, 1.2f, 3)
            ),
            Optional.empty(),
            SHIELD_BREAK_AUDIO_PRIORITY,
            AudioAttenuation.WORLD_3D,
            AudioCategory.PLAYERS,
            AudioBus.COMBAT
        );
        return new AudioEventPayload.PlaySoundRecipe(
            "shield_break_bone",
            AUDIO_INSTANCE_SEQ.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        );
    }
}
