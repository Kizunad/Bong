package com.bong.client.craft;

import com.bong.client.visual.particle.BongParticles;
import com.bong.client.visual.particle.BongSpriteParticle;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.math.BlockPos;

import java.util.List;

/** plan-workbench-place-runtime-v1 P2 — 制作台落座尘土粒子。 */
public final class WorkbenchPlaceDust {
    public static final int RGB = 0x8B7355;
    public static final int LIFETIME_TICKS = 12;
    public static final float ALPHA = 0.65f;
    public static final float SCALE = 0.12f;
    private static final String WORKBENCH_ITEM_ID = "workbench_item";
    private static final String TRADE_CRATE_ITEM_ID = "trade_crate";
    private static final String HERB_CRATE_PLACED_ITEM_ID = "herb_crate_placed";
    private static final String DEAD_DROP_BOX_ITEM_ID = "dead_drop_box";

    private WorkbenchPlaceDust() {
    }

    public static List<ParticleSpec> plan(BlockPos pos) {
        double cx = pos.getX() + 0.5;
        double cy = pos.getY() + 0.1;
        double cz = pos.getZ() + 0.5;
        return List.of(
            new ParticleSpec(cx + 0.18, cy, cz + 0.06, 0.045, -0.006, 0.015),
            new ParticleSpec(cx - 0.14, cy, cz + 0.16, -0.035, -0.005, 0.040),
            new ParticleSpec(cx + 0.08, cy, cz - 0.18, 0.020, -0.007, -0.045),
            new ParticleSpec(cx - 0.16, cy, cz - 0.08, -0.040, -0.006, -0.020)
        );
    }

    public static boolean shouldSpawnForItem(String itemId) {
        return WORKBENCH_ITEM_ID.equals(itemId)
            || TRADE_CRATE_ITEM_ID.equals(itemId)
            || HERB_CRATE_PLACED_ITEM_ID.equals(itemId)
            || DEAD_DROP_BOX_ITEM_ID.equals(itemId);
    }

    public static void spawn(MinecraftClient client, BlockPos pos) {
        if (client == null || client.world == null || client.particleManager == null || pos == null) {
            return;
        }
        ClientWorld world = client.world;
        SpriteProvider provider = spriteProvider();
        for (ParticleSpec spec : plan(pos)) {
            if (provider == null) {
                world.addParticle(
                    BongParticles.CLOUD_DUST,
                    spec.x(), spec.y(), spec.z(),
                    spec.vx(), spec.vy(), spec.vz()
                );
                continue;
            }
            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                spec.x(), spec.y(), spec.z(),
                spec.vx(), spec.vy(), spec.vz()
            );
            particle.setSpritePublic(provider.getSprite(world.random));
            particle.setColor(0.545f, 0.451f, 0.333f);
            particle.setAlphaPublic(ALPHA);
            particle.setMaxAgePublic(LIFETIME_TICKS);
            particle.setScalePublic(SCALE);
            client.particleManager.addParticle(particle);
        }
    }

    private static SpriteProvider spriteProvider() {
        if (BongParticles.cloudDustSprites != null) {
            return BongParticles.cloudDustSprites;
        }
        if (BongParticles.enlightenmentDustSprites != null) {
            return BongParticles.enlightenmentDustSprites;
        }
        return BongParticles.qiAuraSprites;
    }

    public record ParticleSpec(double x, double y, double z, double vx, double vy, double vz) {
    }
}
