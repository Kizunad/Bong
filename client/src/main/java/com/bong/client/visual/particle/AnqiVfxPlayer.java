package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;

/**
 * 暗器六招的客户端粒子播放器（封骨 / 单射狙击 / 多发齐射 / 凝魂注射 / 破甲注射 / 诱饵分形）。
 *
 * <p>与 {@link SwordPathVfxPlayer} 同模式：单实例服务多个 event_id，按 event_id 路由到
 * 既有的 {@link BongParticles} sprite——<b>不引入任何新粒子贴图</b>。所有 sprite 在
 * {@code BongParticles.registerClient} 已注册。
 *
 * <p>event_id → 视觉语义 / 复用 sprite：
 * <ul>
 *   <li>{@code anqi_charge_seal}  封骨密封 → tribulationSpark（骨白封印火花）</li>
 *   <li>{@code anqi_snipe_bolt}   单射弹道 → swordQiTrail line（朝向弹道，复用剑气线条）</li>
 *   <li>{@code anqi_multi_volley} 扇形齐射 → flyingSwordTrail（散射弹幕拖尾）</li>
 *   <li>{@code anqi_soul_inject}  凝魂注射 → duguDarkGreenMist（魂注紫雾）</li>
 *   <li>{@code anqi_armor_pierce} 破甲火花 → tieBiMetallic（朝向金属迸溅）</li>
 *   <li>{@code anqi_echo_decoy}   分形回响 → lingqiRipple decal（分身涟漪）</li>
 * </ul>
 *
 * <p>由 server {@code network::vfx_animation_trigger::emit_anqi_visual_triggers} 在暗器招式
 * cast 成功后 emit 对应 event_id（含 origin / direction / color / count / duration）。
 */
public final class AnqiVfxPlayer implements VfxPlayer {
    public static final Identifier ANQI_CHARGE_SEAL = id("anqi_charge_seal");
    public static final Identifier ANQI_SNIPE_BOLT = id("anqi_snipe_bolt");
    public static final Identifier ANQI_MULTI_VOLLEY = id("anqi_multi_volley");
    public static final Identifier ANQI_SOUL_INJECT = id("anqi_soul_inject");
    public static final Identifier ANQI_ARMOR_PIERCE = id("anqi_armor_pierce");
    public static final Identifier ANQI_ECHO_DECOY = id("anqi_echo_decoy");

    public static final List<Identifier> EVENT_IDS = List.of(
        ANQI_CHARGE_SEAL,
        ANQI_SNIPE_BOLT,
        ANQI_MULTI_VOLLEY,
        ANQI_SOUL_INJECT,
        ANQI_ARMOR_PIERCE,
        ANQI_ECHO_DECOY
    );

    private static Identifier id(String path) {
        return new Identifier("bong", path);
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        Identifier eventId = payload.eventId();
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb(eventId));
        int count = GameplayVfxUtil.count(payload, fallbackCount(eventId), 1, 48);
        int duration = GameplayVfxUtil.duration(payload, fallbackDuration(eventId));

        // 分形回响：地面涟漪 decal（分身散开的扩散环）。
        if (eventId.equals(ANQI_ECHO_DECOY)) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
                ox, oy + 0.03, oz, rgb, 0.72f, duration, 1.6);
            return;
        }

        // 朝向类（弹道 / 破甲）：沿 caster→target 方向画 line trail。
        if (eventId.equals(ANQI_SNIPE_BOLT) || eventId.equals(ANQI_ARMOR_PIERCE)) {
            double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
            SpriteProvider lineSprite = eventId.equals(ANQI_ARMOR_PIERCE)
                ? BongParticles.tieBiMetallicSprites
                : BongParticles.swordQiTrailSprites;
            GameplayVfxUtil.spawnLine(client, world, lineSprite,
                ox, oy + 1.0, oz, dir[0] * 1.4, dir[1] * 1.4, dir[2] * 1.4,
                rgb, 0.9f, duration, 0.12);
            return;
        }

        // 其余（封骨 / 齐射 / 魂注）：散布 sprite burst。
        SpriteProvider provider = spriteProviderFor(eventId);
        for (int i = 0; i < count; i++) {
            double vx = (world.random.nextDouble() - 0.5) * velocityScale(eventId);
            double vy = world.random.nextDouble() * velocityScale(eventId) + 0.02;
            double vz = (world.random.nextDouble() - 0.5) * velocityScale(eventId);
            GameplayVfxUtil.spawnSprite(client, world, provider,
                ox + (world.random.nextDouble() - 0.5) * 0.4,
                oy + world.random.nextDouble() * 0.9,
                oz + (world.random.nextDouble() - 0.5) * 0.4,
                vx, vy, vz, rgb, 0.82f, duration, scaleFor(eventId));
        }
    }

    private static SpriteProvider spriteProviderFor(Identifier eventId) {
        if (eventId.equals(ANQI_SOUL_INJECT)) {
            return BongParticles.duguDarkGreenMistSprites;
        }
        if (eventId.equals(ANQI_MULTI_VOLLEY)) {
            return BongParticles.flyingSwordTrailSprites;
        }
        // 封骨密封 → 火花。
        return BongParticles.tribulationSparkSprites;
    }

    private static int fallbackRgb(Identifier eventId) {
        if (eventId.equals(ANQI_SOUL_INJECT) || eventId.equals(ANQI_ECHO_DECOY)) {
            return 0xB9A7FF;
        }
        if (eventId.equals(ANQI_ARMOR_PIERCE)) {
            return 0xC0C4C8;
        }
        if (eventId.equals(ANQI_SNIPE_BOLT)) {
            return 0xC8E0F0;
        }
        if (eventId.equals(ANQI_MULTI_VOLLEY)) {
            return 0xD8E0B0;
        }
        // 封骨——骨白。
        return 0xE8DCC8;
    }

    private static int fallbackCount(Identifier eventId) {
        if (eventId.equals(ANQI_MULTI_VOLLEY)) {
            return 20;
        }
        if (eventId.equals(ANQI_SOUL_INJECT)) {
            return 16;
        }
        return 12;
    }

    private static int fallbackDuration(Identifier eventId) {
        if (eventId.equals(ANQI_ECHO_DECOY)) {
            return 30;
        }
        if (eventId.equals(ANQI_SOUL_INJECT)) {
            return 26;
        }
        return 22;
    }

    private static double velocityScale(Identifier eventId) {
        if (eventId.equals(ANQI_MULTI_VOLLEY)) {
            return 0.22;
        }
        if (eventId.equals(ANQI_SOUL_INJECT)) {
            return 0.10;
        }
        return 0.14;
    }

    private static float scaleFor(Identifier eventId) {
        if (eventId.equals(ANQI_SOUL_INJECT)) {
            return 0.12f;
        }
        return 0.10f;
    }
}
