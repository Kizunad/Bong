package com.bong.client.visual.particle;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.render.model.json.ModelTransformationMode;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

/**
 * Client-only world VFX demo harness for plan-particle-system-v1 §3 / §5.3.
 *
 * <p>The demo is intentionally not an authoritative gameplay entity or block
 * entity. One vfx event starts a short-lived local actor; movement, ribbon
 * trails, and ground decals then self-play on the client so the high-frequency
 * frames do not occupy the {@code bong:vfx_event} channel.</p>
 */
public final class WorldVfxDemoBootstrap {
    private static final List<FlyingSwordDemoState> FLYING_SWORDS = new ArrayList<>();
    private static final List<FormationCoreDemoState> FORMATION_CORES = new ArrayList<>();
    private static final ItemStack FLYING_SWORD_STACK = new ItemStack(Items.DIAMOND_SWORD);
    private static ClientWorld activeWorld;

    private WorldVfxDemoBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(WorldVfxDemoBootstrap::tick);
        WorldRenderEvents.AFTER_ENTITIES.register(WorldVfxDemoBootstrap::render);
    }

    public static void spawnFlyingSwordDemo(
        ClientWorld world,
        Vec3d origin,
        Vec3d direction,
        int durationTicks,
        double strength,
        int colorRgb
    ) {
        ensureWorld(world);
        FLYING_SWORDS.add(new FlyingSwordDemoState(origin, direction, durationTicks, strength, colorRgb));
    }

    public static void spawnFormationCoreDemo(
        ClientWorld world,
        Vec3d origin,
        int durationTicks,
        double strength,
        int colorRgb
    ) {
        ensureWorld(world);
        FormationCoreDemoState state = new FormationCoreDemoState(origin, durationTicks, strength, colorRgb);
        FORMATION_CORES.add(state);
    }

    private static void tick(MinecraftClient client) {
        ClientWorld world = client.world;
        if (world == null) {
            clear();
            return;
        }
        ensureWorld(world);

        Iterator<FlyingSwordDemoState> swordIt = FLYING_SWORDS.iterator();
        while (swordIt.hasNext()) {
            FlyingSwordDemoState state = swordIt.next();
            spawnRibbonTrail(world, state);
            if (!state.tick()) {
                swordIt.remove();
            }
        }

        Iterator<FormationCoreDemoState> formationIt = FORMATION_CORES.iterator();
        while (formationIt.hasNext()) {
            FormationCoreDemoState state = formationIt.next();
            if (state.shouldPulse()) {
                spawnFormationPulse(world, state);
            }
            if (!state.tick()) {
                formationIt.remove();
            }
        }
    }

    private static void render(WorldRenderContext context) {
        if (FLYING_SWORDS.isEmpty()) {
            return;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        ClientWorld world = client.world;
        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (world == null || consumers == null || matrices == null) {
            return;
        }

        Vec3d cam = context.camera().getPos();
        float tickDelta = context.tickDelta();
        for (FlyingSwordDemoState state : FLYING_SWORDS) {
            Vec3d pos = state.position(tickDelta);
            BlockPos lightPos = BlockPos.ofFloored(pos.x, pos.y, pos.z);
            int light = WorldRenderer.getLightmapCoordinates(world, lightPos);
            float yaw = (float) Math.toDegrees(Math.atan2(state.direction.z, state.direction.x));
            float roll = (float) (Math.sin((state.ageTicks + tickDelta) * 0.25) * 12.0);

            matrices.push();
            matrices.translate(pos.x - cam.x, pos.y - cam.y, pos.z - cam.z);
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(90.0f - yaw));
            matrices.multiply(RotationAxis.POSITIVE_Z.rotationDegrees(roll));
            client.getItemRenderer().renderItem(
                FLYING_SWORD_STACK,
                ModelTransformationMode.GROUND,
                light,
                OverlayTexture.DEFAULT_UV,
                matrices,
                consumers,
                world,
                state.ageTicks
            );
            matrices.pop();
        }
    }

    private static void spawnRibbonTrail(ClientWorld world, FlyingSwordDemoState state) {
        Vec3d prev = state.previousPosition();
        Vec3d now = state.position(0.0f);
        Vec3d velocity = now.subtract(prev);
        BongRibbonParticle particle = new BongRibbonParticle(
            world,
            prev.x,
            prev.y,
            prev.z,
            velocity.x,
            velocity.y,
            velocity.z,
            8
        );
        float[] rgb = rgb(state.colorRgb);
        particle.setRibbonWidth(0.10 + 0.06 * state.strength, 0.01)
            .setColor(rgb[0], rgb[1], rgb[2]);
        particle.setAlphaPublic((float) (0.45 + 0.35 * state.strength));
        particle.setMaxAgePublic(18);
        if (BongParticles.flyingSwordTrailSprites != null) {
            particle.setSpritePublic(BongParticles.flyingSwordTrailSprites.getSprite(world.random));
        }
        MinecraftClient.getInstance().particleManager.addParticle(particle);
    }

    private static void spawnFormationPulse(ClientWorld world, FormationCoreDemoState state) {
        if (state.pulseAgeTicks() <= 0) {
            return;
        }
        BongGroundDecalParticle particle = new BongGroundDecalParticle(
            world,
            state.origin.x,
            state.origin.y,
            state.origin.z
        );
        float[] rgb = rgb(state.colorRgb);
        particle.setDecalShape(state.halfSize(), 0.025)
            .setSpin(world.random.nextDouble() * Math.PI * 2.0, 0.055)
            .setColor(rgb[0], rgb[1], rgb[2]);
        particle.setAlphaPublic((float) (0.35 + 0.45 * state.strength));
        particle.setMaxAgePublic(state.pulseAgeTicks());
        if (BongParticles.lingqiRippleSprites != null) {
            particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        MinecraftClient.getInstance().particleManager.addParticle(particle);
    }

    private static void ensureWorld(ClientWorld world) {
        if (activeWorld != world) {
            clear();
            activeWorld = world;
        }
    }

    private static void clear() {
        FLYING_SWORDS.clear();
        FORMATION_CORES.clear();
        activeWorld = null;
    }

    private static float[] rgb(int colorRgb) {
        return new float[] {
            ((colorRgb >> 16) & 0xFF) / 255.0f,
            ((colorRgb >> 8) & 0xFF) / 255.0f,
            (colorRgb & 0xFF) / 255.0f,
        };
    }
}
