package com.bong.client.alchemy;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.inspect.ItemInspectModel;
import com.bong.client.visual.particle.BongParticles;
import com.bong.client.visual.particle.BongSpriteParticle;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/** 世界丹炉的有限寿命表现。由服务端广播驱动，与 Inspect 和工位窗口的生命周期无关。 */
public final class AlchemyWorldEffects {
    private static final Map<BlockPos, FurnaceState> FURNACES = new HashMap<>();
    /** 当前世界中可按方块坐标直达的丹炉模型，供瞬时 VFX 使用。 */
    private static final Map<BlockPos, BongModeledEntity> FURNACE_MODELS = new HashMap<>();
    private static final List<ActionEffect> ACTIONS = new ArrayList<>();
    private static ClientWorld world;
    private static long tick;

    private AlchemyWorldEffects() {}

    private record FurnaceState(AlchemyWorldPayload payload, long expiresAt, int smokeColor) {}

    static final class ActionEffect {
        final AlchemyWorldPayload payload;
        final long start;
        final int duration;
        final ItemInspectModel model;
        boolean animated;

        ActionEffect(AlchemyWorldPayload payload) {
            this.payload = payload;
            start = tick;
            duration = payload.action().equals("collect") ? 48 : 18;
            model = payload.item().isEmpty() ? null : ItemInspectModel.find(payload.item()).orElse(null);
        }

        float age(float delta) { return tick - start + delta; }
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(AlchemyWorldEffects::tick);
        WorldRenderEvents.AFTER_ENTITIES.register(context ->
            AlchemyWorldRenderer.render(context, world, List.copyOf(ACTIONS)));
    }

    private static void ensureWorld(ClientWorld next) {
        if (world == next) return;
        FURNACES.clear();
        resetFurnaceModels();
        ACTIONS.clear();
        world = next;
        tick = 0;
    }

    private static void resetFurnaceModels() {
        resetFurnaceModels(FURNACE_MODELS, BongModeledEntity::resetAlchemyEffects);
    }

    /** 重置已登记模型后再丢弃索引，避免最后一帧的热度或过渡动画残留。 */
    static <T> void resetFurnaceModels(Map<BlockPos, T> models, Consumer<? super T> resetter) {
        Throwable primary = null;
        for (T model : models.values()) {
            try {
                resetter.accept(model);
            } catch (RuntimeException | Error failure) {
                primary = accumulate(primary, failure);
            }
        }
        try {
            models.clear();
        } catch (RuntimeException | Error failure) {
            primary = accumulate(primary, failure);
        }
        if (primary instanceof RuntimeException failure) throw failure;
        if (primary instanceof Error failure) throw failure;
    }

    private static Throwable accumulate(Throwable primary, Throwable failure) {
        if (primary == null) return failure;
        if (primary != failure) primary.addSuppressed(failure);
        return primary;
    }

    /** 统一 ServerDataRouter 已在客户端线程完成连接代际校验。 */
    public static void accept(MinecraftClient client, AlchemyWorldPayload payload) {
        ensureWorld(client.world);
        if (world == null || !world.getRegistryKey().equals(net.minecraft.world.World.OVERWORLD)) return;
        FURNACES.put(payload.position(), new FurnaceState(payload, tick + 40, materialColor(payload.materials())));
        if (!payload.action().equals("state")) {
            if (ACTIONS.size() >= 128) ACTIONS.remove(0);
            ACTIONS.add(new ActionEffect(payload));
        }
    }

    /** 炸炉等瞬时表现按炉位查找模型，避免每个事件重新扫描世界实体。 */
    public static BongModeledEntity furnaceAt(BlockPos position) {
        var furnace = FURNACE_MODELS.get(position);
        if (furnace == null || furnace.isRemoved()) {
            if (furnace != null) FURNACE_MODELS.remove(position);
            return null;
        }
        return furnace;
    }

    private static int materialColor(Map<String, Integer> materials) {
        long red = 0, green = 0, blue = 0, total = 0;
        for (var entry : materials.entrySet()) {
            int rgb = AlchemyMaterialColors.color(entry.getKey());
            long count = entry.getValue();
            red += ((rgb >> 16) & 255) * count;
            green += ((rgb >> 8) & 255) * count;
            blue += (rgb & 255) * count;
            total += count;
        }
        return total == 0 ? 0xAAA69A : (int) (red / total) << 16 | (int) (green / total) << 8 | (int) (blue / total);
    }

    private static void tick(MinecraftClient client) {
        ensureWorld(client.world);
        if (world == null || client.player == null) return;
        tick++;
        FURNACES.entrySet().removeIf(entry -> entry.getValue().expiresAt() <= tick
            || client.player.squaredDistanceTo(Vec3d.ofCenter(entry.getKey())) > 48 * 48);
        if (FURNACES.isEmpty() && ACTIONS.isEmpty()) {
            resetFurnaceModels();
            return;
        }
        Map<BlockPos, BongModeledEntity> models = new HashMap<>();
        for (var entity : world.getEntities()) {
            if (entity instanceof BongModeledEntity modeled && !entity.isRemoved()
                && modeled.modelKind() == BongEntityModelKind.ALCHEMY_FURNACE) {
                models.put(entity.getBlockPos(), modeled);
            }
        }
        FURNACE_MODELS.clear();
        FURNACE_MODELS.putAll(models);
        for (var entry : FURNACES.entrySet()) {
            var model = models.get(entry.getKey());
            if (model == null) continue;
            var state = entry.getValue();
            model.setAlchemyHeat((float) state.payload().heat());
            emitAmbient(client, entry.getKey(), state);
        }
        // 移除状态后也必须停止炉身震动，不能把最后收到的高火候永久留在模型上。
        models.forEach((pos, model) -> { if (!FURNACES.containsKey(pos)) model.setAlchemyHeat(0); });
        ACTIONS.removeIf(effect -> effect.age(0) >= effect.duration
            || (effect.animated && !models.containsKey(effect.payload.position())));
        for (var effect : ACTIONS) {
            var model = models.get(effect.payload.position());
            if (model == null) continue;
            if (!effect.animated) {
                if (effect.payload.action().equals("feed")) model.playAlchemyFeed();
                if (effect.payload.action().equals("collect")) {
                    model.playAlchemyResult(resultEffect(effect.payload.result()));
                }
                effect.animated = true;
            }
            emitAction(client, effect);
        }
    }

    static AlchemyResultEffect resultEffect(String result) {
        return AlchemyResultEffect.fromOutcome(result.equals("early_take") ? "waste" : result, result.equals("early_take"));
    }

    // 炉底火焰和炉口彩烟按热度连续产生；声音是短段燃烧声，无需维护永久循环。
    private static void emitAmbient(MinecraftClient client, BlockPos pos, FurnaceState state) {
        double heat = state.payload().heat();
        Vec3d center = Vec3d.ofBottomCenter(pos);
        if (heat > .01 && tick % 2 == 0) {
            for (int index = 0; index < 1 + (int) (heat * 5); index++) {
                double angle = world.random.nextDouble() * Math.PI * 2;
                double radius = .18 + world.random.nextDouble() * .16;
                world.addParticle(ParticleTypes.FLAME, center.x + Math.cos(angle) * radius,
                    center.y + .22, center.z + Math.sin(angle) * radius, 0, .01 + heat * .025, 0);
            }
            sprite(client, BongParticles.cloudDustSprites, center.add(jitter(.12), 1.3, jitter(.12)),
                new Vec3d(jitter(.004), .012 + heat * .025, jitter(.004)), state.smokeColor(), .35f, 25 + (int) (heat * 20), 1.5f);
            if (heat > .65) world.addParticle(ParticleTypes.LAVA, center.x, center.y + .3, center.z, 0, 0, 0);
        }
        if (heat > .01 && Math.floorMod(tick + pos.asLong(), 38) == 0) {
            AlchemySoundscape.playFire(world, center, (float) heat);
        }
        if (state.payload().incense() && tick % 5 == 0) {
            sprite(client, BongParticles.cloudDustSprites, center.add(-.65, .65, .1),
                new Vec3d(.001, .02, .001), 0xBCB6AA, .25f, 38, .6f);
        }
    }

    private static void emitAction(MinecraftClient client, ActionEffect effect) {
        var payload = effect.payload;
        Vec3d mouth = Vec3d.ofBottomCenter(payload.position()).add(0, 1.2, 0);
        float age = effect.age(0);
        switch (payload.action()) {
            case "inject_qi" -> {
                // 三股螺旋光流从施术者手边收束进炉身，尾端留一道细环。
                double progress = age / effect.duration;
                for (int strand = 0; strand < 3; strand++) {
                    double angle = progress * Math.PI * 4 + strand * Math.PI * 2 / 3;
                    double radius = Math.sin(progress * Math.PI) * .2;
                    Vec3d point = payload.source().lerp(mouth, progress).add(Math.cos(angle) * radius, Math.sin(angle) * radius, 0);
                    sprite(client, BongParticles.qiAuraSprites, point, Vec3d.ZERO, 0xB789FF, .8f, 10, .9f);
                }
            }
            case "fire_raise", "ignite" -> {
                if (age < 8) world.addParticle(ParticleTypes.FLAME, mouth.x + jitter(.25), mouth.y - .85,
                    mouth.z + jitter(.25), 0, .06, 0);
            }
            case "fire_lower" -> {
                if (age < 7) sprite(client, BongParticles.cloudDustSprites, mouth, new Vec3d(0, .025, 0), 0xBBB7AD, .45f, 18, 1.6f);
            }
            case "feed" -> {
                if (age >= 8) sprite(client, BongParticles.cloudDustSprites, mouth, new Vec3d(jitter(.01), .04, jitter(.01)),
                    AlchemyMaterialColors.color(payload.item()), .45f, 24, 1.3f);
            }
            case "collect" -> {
                if (age > 12) return;
                boolean explode = payload.result().equals("explode");
                if (explode && age == 1) world.addParticle(ParticleTypes.EXPLOSION, mouth.x, mouth.y, mouth.z, 0, 0, 0);
                boolean success = payload.result().equals("perfect") || payload.result().equals("good");
                sprite(client, success ? BongParticles.enlightenmentDustSprites : BongParticles.cloudDustSprites,
                    mouth.add(jitter(.25), 0, jitter(.25)), new Vec3d(jitter(explode ? .12 : .02), .045, jitter(explode ? .12 : .02)),
                    success ? 0xFFE497 : 0x726658, .65f, 25, explode ? 3f : 1.3f);
            }
            default -> { }
        }
    }

    private static double jitter(double radius) { return (world.random.nextDouble() * 2 - 1) * radius; }

    private static void sprite(MinecraftClient client, SpriteProvider sprites, Vec3d point, Vec3d velocity,
                               int rgb, float alpha, int duration, float scale) {
        if (sprites == null || client.particleManager == null) return;
        var particle = new BongSpriteParticle(world, point.x, point.y, point.z, velocity.x, velocity.y, velocity.z);
        particle.setSpritePublic(sprites.getSprite(world.random)).setAlphaPublic(alpha).setMaxAgePublic(duration).setScalePublic(scale);
        particle.setColor((rgb >> 16 & 255) / 255f, (rgb >> 8 & 255) / 255f, (rgb & 255) / 255f);
        client.particleManager.addParticle(particle);
    }
}
