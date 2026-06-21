package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;

/**
 * 蛊道（独孤毒流）基础两招的客户端粒子播放器（凝针 / 灌毒蛊）。
 *
 * <p>与 {@link AnqiVfxPlayer} 同模式：单实例服务多个 event_id，按 event_id 路由到既有的
 * {@link BongParticles} sprite——<b>不引入任何新粒子贴图</b>。所有 sprite 在
 * {@code BongParticles.registerClient} 已注册。
 *
 * <p>event_id → 视觉语义 / 复用 sprite：
 * <ul>
 *   <li>{@code dugu_needle_bolt}   凝针弹道 → swordQiTrail line（沿 caster→target 朝向画
 *       细针直刺线条，冷青绿；复用剑气线条 sprite）</li>
 *   <li>{@code dugu_poison_infuse} 灌毒蛊 → duguDarkGreenMist（失谐真元毒绿雾绕身散布，
 *       覆入飞针的视觉前奏）</li>
 * </ul>
 *
 * <p>由 server {@code network::vfx_animation_trigger::emit_dugu_needle_visual_triggers} 在蛊道
 * 招式 cast 成功后 emit 对应 event_id（含 origin / direction / color / count / duration）。
 *
 * <p><b>event_id 与 server 端 {@code VFX_DUGU_NEEDLE_BOLT} / {@code VFX_DUGU_POISON_INFUSE}
 * 逐字符对齐。</b>
 */
public final class DuguNeedleVfxPlayer implements VfxPlayer {
    public static final Identifier DUGU_NEEDLE_BOLT = id("dugu_needle_bolt");
    public static final Identifier DUGU_POISON_INFUSE = id("dugu_poison_infuse");

    public static final List<Identifier> EVENT_IDS = List.of(
        DUGU_NEEDLE_BOLT,
        DUGU_POISON_INFUSE
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

        // 凝针：沿 caster→target 方向画 line trail（细针直刺）。
        if (eventId.equals(DUGU_NEEDLE_BOLT)) {
            double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
            GameplayVfxUtil.spawnLine(client, world, BongParticles.swordQiTrailSprites,
                ox, oy + 1.0, oz, dir[0] * 1.4, dir[1] * 1.4, dir[2] * 1.4,
                rgb, 0.9f, duration, 0.10);
            return;
        }

        // 灌毒蛊：失谐真元毒绿雾绕身散布（覆入飞针前奏）。
        SpriteProvider provider = BongParticles.duguDarkGreenMistSprites;
        for (int i = 0; i < count; i++) {
            double vx = (world.random.nextDouble() - 0.5) * 0.10;
            double vy = world.random.nextDouble() * 0.10 + 0.02;
            double vz = (world.random.nextDouble() - 0.5) * 0.10;
            GameplayVfxUtil.spawnSprite(client, world, provider,
                ox + (world.random.nextDouble() - 0.5) * 0.5,
                oy + world.random.nextDouble() * 1.0,
                oz + (world.random.nextDouble() - 0.5) * 0.5,
                vx, vy, vz, rgb, 0.82f, duration, 0.12f);
        }
    }

    private static int fallbackRgb(Identifier eventId) {
        if (eventId.equals(DUGU_NEEDLE_BOLT)) {
            // 真元凝针——冷青绿（带毒流底色，区别于暗器纯青白弹道）。
            return 0xBFE3D0;
        }
        // 灌毒蛊——失谐真元毒绿。
        return 0x44AA44;
    }

    private static int fallbackCount(Identifier eventId) {
        if (eventId.equals(DUGU_NEEDLE_BOLT)) {
            return 10;
        }
        return 14;
    }

    private static int fallbackDuration(Identifier eventId) {
        if (eventId.equals(DUGU_NEEDLE_BOLT)) {
            return 16;
        }
        return 24;
    }
}
