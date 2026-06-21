package com.bong.client.network;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.visual.particle.BongVfxParticleBridge;
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
 * plan-shield-block-v1 §P4：{@code shield_block_hit} payload 客户端 handler。
 *
 * <p>格挡命中发生时，server emit {@code shield_block_hit} 包含 {@code template_id}（wooden_shield /
 * bone_shield），client 按材质差异化触发命中粒子 + 命中音效：
 * <ul>
 *   <li>木盾 → {@code bong:shield_block_wood} 粒子 + {@code shield_block.json} recipe（pitch 0.9）</li>
 *   <li>骨盾 → {@code bong:shield_block_bone} 粒子 + {@code shield_block_bone.json} recipe（pitch 1.3 + skeleton.hurt 第二层）</li>
 * </ul>
 *
 * <p>命中粒子与 P3 破盾粒子区分：
 * <ul>
 *   <li>P3 破盾：{@code bong:shield_break_wood} / {@code bong:shield_break_bone}，count 7，duration 10t</li>
 *   <li>P4 命中：{@code bong:shield_block_wood} / {@code bong:shield_block_bone}，count 6，duration 8t（更轻量）</li>
 * </ul>
 *
 * <p>HUD 事件流：通过 {@link ServerDataDispatch#handledWithEventAlert} 的 ToastSpec 机制，
 * 在现有事件流上追加一条格挡提示（复用 handledWithEventAlert 基础设施，非新 layer）。
 * 颜色 {@link #BLOCK_HUD_COLOR} 区别于普通受击 {@code 0xFFC04040}。
 *
 * <p>未持盾时 server 不会 emit 此 payload，因此 HUD conditional 由 server 端保证——
 * client 不需要本地过滤。
 */
public final class ShieldBlockHitHandler implements ServerDataHandler {

    /**
     * 格挡 HUD 浮字颜色：青蓝盾色，区别于普通受击红（{@code 0xFFC04040}）和破盾红。
     * 让玩家可感知"这是格挡而非普通受击"。
     */
    static final int BLOCK_HUD_COLOR = 0xFF4DA6FF;

    /** 格挡 HUD 提示时长（ms）。 */
    static final long BLOCK_HUD_DURATION_MS = 1_500L;

    /** 盾弧瞬态确认时长（ms）：比 toast 更短，命中瞬间闪一下即隐藏（条件/瞬态硬约束）。 */
    static final long SHIELD_BLOCK_HUD_FLASH_MS = 350L;

    /** 命中迸溅粒子 vfx_event id —— 木盾，与 VfxBootstrap 注册键一致。 */
    static final Identifier PARTICLE_BLOCK_WOOD = new Identifier("bong", "shield_block_wood");

    /** 命中迸溅粒子 vfx_event id —— 骨盾。 */
    static final Identifier PARTICLE_BLOCK_BONE = new Identifier("bong", "shield_block_bone");

    private static final int SHIELD_BLOCK_AUDIO_PRIORITY = 68;
    private static final AtomicLong AUDIO_INSTANCE_SEQ = new AtomicLong(95_000L);

    /** 注入点，测试时替换为 stub。 */
    private static AudioPlaybackBridge audioBridgeOverride = null;

    /** 粒子 bridge 注入点，测试时替换为 stub。 */
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
        String templateId = payload.has("template_id")
            ? payload.get("template_id").getAsString() : "unknown";

        triggerMaterialEffects(templateId);

        // HUD：准星下方瞬态盾弧确认（条件式——仅命中瞬间闪一下，~350ms 后自动隐藏，绝不常驻）。
        com.bong.client.hud.ZhenmaiHudStateStore.flashShieldBlock(
            System.currentTimeMillis(), SHIELD_BLOCK_HUD_FLASH_MS
        );

        String toastText = "bone_shield".equals(templateId) ? "骨盾架住" : "盾格挡";
        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            new ServerDataDispatch.ToastSpec(toastText, BLOCK_HUD_COLOR, BLOCK_HUD_DURATION_MS),
            null,
            "Shield block hit: template=" + templateId
        );
    }

    /**
     * 按 template_id 触发命中粒子 + 音效（材质差异化）。
     */
    private void triggerMaterialEffects(String templateId) {
        AudioPlaybackBridge audio = audioBridgeOverride != null
            ? audioBridgeOverride : SoundRecipePlayer.instance();
        VfxParticleBridge particles = particleBridgeOverride != null
            ? particleBridgeOverride : new BongVfxParticleBridge();

        if ("wooden_shield".equals(templateId)) {
            audio.play(woodBlockRecipePayload());
            spawnBlockParticle(particles, PARTICLE_BLOCK_WOOD);
        } else if ("bone_shield".equals(templateId)) {
            audio.play(boneBlockRecipePayload());
            spawnBlockParticle(particles, PARTICLE_BLOCK_BONE);
        } else {
            // 未知材质降级：木盾效果
            audio.play(woodBlockRecipePayload());
            spawnBlockParticle(particles, PARTICLE_BLOCK_WOOD);
        }
    }

    /**
     * 在本地玩家位置触发命中粒子（count=6，duration=8t，轻于破盾 count=7，duration=10t）。
     */
    private static void spawnBlockParticle(VfxParticleBridge bridge, Identifier eventId) {
        double[] origin = localPlayerOrigin();
        VfxEventPayload.SpawnParticle particlePayload = new VfxEventPayload.SpawnParticle(
            eventId,
            origin,
            Optional.empty(),
            OptionalInt.empty(),
            Optional.empty(),
            OptionalInt.of(6),
            OptionalInt.of(8)
        );
        bridge.spawnParticle(particlePayload);
    }

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
        return new double[]{pos.x, pos.y + 0.9, pos.z};
    }

    // ---- 音效内联 recipe -----------------------------------------------

    /**
     * 木盾格挡命中音效：{@code item.shield.block} pitch 0.9，与骨盾 pitch 1.3 形成可感知差异。
     */
    private static AudioEventPayload.PlaySoundRecipe woodBlockRecipePayload() {
        AudioRecipe recipe = new AudioRecipe(
            "shield_block",
            List.of(
                new AudioLayer(new Identifier("minecraft", "item.shield.block"), 0.9f, 0.9f, 0)
            ),
            Optional.empty(),
            SHIELD_BLOCK_AUDIO_PRIORITY,
            AudioAttenuation.WORLD_3D,
            AudioCategory.PLAYERS,
            AudioBus.COMBAT
        );
        return new AudioEventPayload.PlaySoundRecipe(
            "shield_block",
            AUDIO_INSTANCE_SEQ.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        );
    }

    /**
     * 骨盾格挡命中音效：layer1 pitch 1.3（脆高音区别木盾），layer2 {@code entity.skeleton.hurt}
     * vol 0.3 delay 1tick（骨感回响，差异化）。
     */
    private static AudioEventPayload.PlaySoundRecipe boneBlockRecipePayload() {
        AudioRecipe recipe = new AudioRecipe(
            "shield_block_bone",
            List.of(
                new AudioLayer(new Identifier("minecraft", "item.shield.block"), 0.8f, 1.3f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.skeleton.hurt"), 0.3f, 1.0f, 1)
            ),
            Optional.empty(),
            SHIELD_BLOCK_AUDIO_PRIORITY,
            AudioAttenuation.WORLD_3D,
            AudioCategory.PLAYERS,
            AudioBus.COMBAT
        );
        return new AudioEventPayload.PlaySoundRecipe(
            "shield_block_bone",
            AUDIO_INSTANCE_SEQ.incrementAndGet(),
            Optional.empty(),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        );
    }
}
