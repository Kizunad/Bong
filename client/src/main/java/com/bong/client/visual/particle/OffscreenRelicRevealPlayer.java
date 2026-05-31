package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * plan-offscreen-war-v1 P3：离屏战场遗物被玩家发现时的地表遗骸贴花（{@code offscreen_relic_reveal}）。
 *
 * <p>server 端 {@code npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system} 在玩家
 * 靠近战场 zone、把 sqlite pending relic 物化成地面 loot 时 emit 本 VFX（每件遗物一处），用一片
 * 贴地 {@link BongGroundDecalParticle} 标出"此地曾有厮杀、散落遗骸"。
 *
 * <p>视觉规格（plan §145）：
 * <ul>
 *   <li>贴图：复用现成地面 decal sprite（{@link BongParticles#lingqiRippleSprites}），不新建贴图；</li>
 *   <li>颜色：骨堆灰白（payload 默认 {@code #B8AFA0}），残卷暗黄留给 server 按 loot 细分时改 color hex；</li>
 *   <li>形态：radial 静态**单**贴花（不自转、不漂移），lifetime 拉满 ~ 持续到拾取的感知近似；</li>
 *   <li>spawn 模式：burst-once-on-hydrate（每次 hydrate 物化一处，不连续刷）。</li>
 * </ul>
 */
public final class OffscreenRelicRevealPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "offscreen_relic_reveal");

    /** 骨堆灰白兜底色（与 server RELIC_DECAL_COLOR_BONE 对齐）。 */
    private static final int FALLBACK_RGB = 0xB8AFA0;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = payload.strength().orElse(0.7);
        int maxAge = payload.durationTicks().orElse(200);
        double halfSize = 0.55 + 0.3 * strength;

        BongGroundDecalParticle decal = new BongGroundDecalParticle(
            world,
            payload.origin()[0],
            payload.origin()[1],
            payload.origin()[2]
        );
        decal.setDecalShape(halfSize, 0.02);
        // 战场遗骸是静止的——不自转（radPerTick=0），随机初始角度避免每处朝向雷同。
        decal.setSpin(world.random.nextDouble() * Math.PI * 2.0, 0.0);
        decal.setColor(r, g, b);
        decal.setAlphaPublic((float) Math.max(0.3, Math.min(0.8, strength)));
        decal.setMaxAgePublic(Math.max(60, Math.min(200, maxAge)));
        if (BongParticles.lingqiRippleSprites != null) {
            decal.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(decal);
    }
}
