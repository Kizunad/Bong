package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;

/**
 * 蛊道 v2 五招（蚀针/自蕴/侵染/神识遮蔽/倒蚀）的客户端粒子播放器。
 *
 * <p>与 {@link DuguNeedleVfxPlayer}（v1 凝针/灌毒蛊）同模式：单实例服务多个 event_id。
 * 三张专属贴图（{@code dugu_taint_pulse} / {@code dugu_dark_green_mist} /
 * {@code dugu_reverse_burst}）自 PR #173 就位、#838 补进图集白名单，但 server 此前从未
 * 把 {@code visual_for()} 的 particle_id 发成 {@code bong:vfx_event}，也没有本注册——
 * 本类与 server {@code emit_dugu_v2_visual_triggers} 一起补齐这条链。
 *
 * <p>event_id → 视觉语义：
 * <ul>
 *   <li>{@code dugu_taint_pulse}（蚀针/侵染命中）毒渍脉冲 ground decal 印于受害者脚下，
 *       外加少量上飘毒渍屑</li>
 *   <li>{@code dugu_dark_green_mist}（神识遮蔽/自蕴）深绿雾绕身缓慢盘旋</li>
 *   <li>{@code dugu_reverse_burst}（倒蚀）亮毒绿放射线束自爆心炸开</li>
 * </ul>
 *
 * <p><b>event_id 与 server {@code combat::dugu_v2::skills::visual_for()} 各招
 * {@code particle_id} 逐字符对齐。</b>
 */
public final class DuguV2VfxPlayer implements VfxPlayer {
    public static final Identifier DUGU_TAINT_PULSE = id("dugu_taint_pulse");
    public static final Identifier DUGU_DARK_GREEN_MIST = id("dugu_dark_green_mist");
    public static final Identifier DUGU_REVERSE_BURST = id("dugu_reverse_burst");

    public static final List<Identifier> EVENT_IDS = List.of(
        DUGU_TAINT_PULSE,
        DUGU_DARK_GREEN_MIST,
        DUGU_REVERSE_BURST
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
        double strength = GameplayVfxUtil.strength(payload, 1.0);

        // 蚀针/侵染：毒渍脉冲贴地印于受害者脚下 + 少量上飘毒渍屑。
        if (eventId.equals(DUGU_TAINT_PULSE)) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.duguTaintPulseSprites,
                ox, oy + 0.05, oz, rgb, 0.85f, duration, 0.55 + strength * 0.35);
            int wisps = Math.max(2, count / 4);
            for (int i = 0; i < wisps; i++) {
                GameplayVfxUtil.spawnSprite(client, world, BongParticles.duguDarkGreenMistSprites,
                    ox + (world.random.nextDouble() - 0.5) * 0.7,
                    oy + 0.15 + world.random.nextDouble() * 0.4,
                    oz + (world.random.nextDouble() - 0.5) * 0.7,
                    (world.random.nextDouble() - 0.5) * 0.02,
                    0.03 + world.random.nextDouble() * 0.03,
                    (world.random.nextDouble() - 0.5) * 0.02,
                    rgb, 0.7f, duration, 0.10f);
            }
            return;
        }

        // 倒蚀：亮毒绿放射线束自爆心炸开（水平偏置的球面随机方向）。
        if (eventId.equals(DUGU_REVERSE_BURST)) {
            for (int i = 0; i < count; i++) {
                double angle = world.random.nextDouble() * Math.PI * 2.0;
                double pitch = (world.random.nextDouble() - 0.5) * 0.9;
                double speed = (0.8 + world.random.nextDouble() * 0.6) * strength;
                GameplayVfxUtil.spawnLine(client, world, BongParticles.duguReverseBurstSprites,
                    ox, oy + 1.0, oz,
                    Math.cos(angle) * Math.cos(pitch) * speed,
                    Math.sin(pitch) * speed,
                    Math.sin(angle) * Math.cos(pitch) * speed,
                    rgb, 0.9f, duration, 0.08);
            }
            return;
        }

        // 神识遮蔽/自蕴：深绿雾绕身缓慢盘旋上浮。
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 0.4 + world.random.nextDouble() * 0.6;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.duguDarkGreenMistSprites,
                ox + Math.cos(angle) * radius,
                oy + world.random.nextDouble() * 1.8,
                oz + Math.sin(angle) * radius,
                -Math.sin(angle) * 0.015,
                0.008 + world.random.nextDouble() * 0.012,
                Math.cos(angle) * 0.015,
                rgb, 0.75f, duration, 0.16f);
        }
    }

    private static int fallbackRgb(Identifier eventId) {
        if (eventId.equals(DUGU_REVERSE_BURST)) {
            // 倒蚀——被引爆的脏真元亮毒绿。
            return 0xA0E070;
        }
        if (eventId.equals(DUGU_TAINT_PULSE)) {
            // 蚀针毒渍——暗绿。
            return 0x57803A;
        }
        // 深绿雾罩。
        return 0x335C41;
    }

    private static int fallbackCount(Identifier eventId) {
        if (eventId.equals(DUGU_REVERSE_BURST)) {
            return 18;
        }
        if (eventId.equals(DUGU_TAINT_PULSE)) {
            return 12;
        }
        return 20;
    }

    private static int fallbackDuration(Identifier eventId) {
        if (eventId.equals(DUGU_REVERSE_BURST)) {
            return 24;
        }
        if (eventId.equals(DUGU_TAINT_PULSE)) {
            return 30;
        }
        return 50;
    }
}
