package com.bong.client.botany;

import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;
import org.joml.Matrix4f;
import software.bernie.geckolib.cache.GeckoLibCache;
import software.bernie.geckolib.cache.object.BakedGeoModel;
import software.bernie.geckolib.core.animatable.GeoAnimatable;
import software.bernie.geckolib.core.animatable.instance.AnimatableInstanceCache;
import software.bernie.geckolib.core.animation.AnimatableManager;
import software.bernie.geckolib.core.animation.AnimationController;
import software.bernie.geckolib.core.animation.RawAnimation;
import software.bernie.geckolib.core.object.Color;
import software.bernie.geckolib.model.GeoModel;
import software.bernie.geckolib.renderer.GeoObjectRenderer;
import software.bernie.geckolib.util.GeckoLibUtil;

import java.util.HashMap;
import java.util.Map;

/** 实体与野生植物心跳共用的 geo 渲染入口；每株植物单独保存动画状态。 */
final class PlantGeoRenderer extends GeoObjectRenderer<PlantGeoRenderer.Instance> {
    private static final PlantGeoRenderer RENDERER = new PlantGeoRenderer();
    private static final Map<String, Instance> INSTANCES = new HashMap<>();
    private static long nextInstanceId;
    private static long lastCleanup;

    private PlantGeoRenderer() {
        super(new Model());
    }

    static boolean render(String key, PlantModelRegistry.PlantStageModel stage, BotanyPlantVisualState visual,
                          long ticks, float tickDelta, MatrixStack matrices,
                          VertexConsumerProvider consumers, int light) {
        if (!GeckoLibCache.getBakedModels().containsKey(stage.geometry())) {
            return false;
        }
        Instance instance = INSTANCES.get(key);
        if (instance == null || instance.stage != stage) {
            instance = new Instance(stage);
            INSTANCES.put(key, instance);
        }
        instance.ticks = ticks;
        instance.visual = visual;
        // 以实际渲染时间清理，世界时间倒退/暂停不会留下永久缓存。
        long now = System.nanoTime();
        instance.lastSeen = now;
        if (now - lastCleanup > 5_000_000_000L) {
            INSTANCES.values().removeIf(value -> now - value.lastSeen > 10_000_000_000L);
            lastCleanup = now;
        }
        RenderLayer layer = RenderLayer.getEntityTranslucent(stage.texture());
        RENDERER.animatable = instance;
        try {
            RENDERER.defaultRender(matrices, instance, consumers, layer, consumers.getBuffer(layer),
                0, tickDelta, light);
        } finally {
            RENDERER.animatable = null;
        }
        return true;
    }

    static void clearCache() {
        INSTANCES.clear();
    }

    @Override
    public long getInstanceId(Instance instance) {
        return instance.id;
    }

    @Override
    public Color getRenderColor(Instance instance, float tickDelta, int light) {
        int rgb = instance.visual.tintRgb();
        return Color.ofRGBA((rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255, instance.visual.alpha());
    }

    @Override
    public void preRender(MatrixStack matrices, Instance instance, BakedGeoModel model,
                          VertexConsumerProvider consumers, VertexConsumer buffer, boolean reRender,
                          float tickDelta, int light, int overlay, float red, float green, float blue, float alpha) {
        // 作者稿已导出到地面中心；不采用 GeoObjectRenderer 默认的半格平移。
        this.objectRenderTranslations = new Matrix4f(matrices.peek().getPositionMatrix());
    }

    static final class Instance implements GeoAnimatable {
        private final long id = ++nextInstanceId;
        private final PlantModelRegistry.PlantStageModel stage;
        private final AnimatableInstanceCache cache = GeckoLibUtil.createInstanceCache(this);
        private BotanyPlantVisualState visual;
        private long ticks;
        private long lastSeen;

        private Instance(PlantModelRegistry.PlantStageModel stage) {
            this.stage = stage;
        }

        @Override
        public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
            if (stage.animation() != null) {
                controllers.add(new AnimationController<>(this, "idle", 0,
                    state -> state.setAndContinue(RawAnimation.begin().thenLoop(stage.animation().idle()))));
            }
        }

        @Override
        public AnimatableInstanceCache getAnimatableInstanceCache() {
            return cache;
        }

        @Override
        public double getTick(Object related) {
            return ticks;
        }
    }

    private static final class Model extends GeoModel<Instance> {
        @Override
        public Identifier getModelResource(Instance instance) {
            return instance.stage.geometry();
        }

        @Override
        public Identifier getTextureResource(Instance instance) {
            return instance.stage.texture();
        }

        @Override
        public Identifier getAnimationResource(Instance instance) {
            return instance.stage.animation() == null ? null : instance.stage.animation().resource();
        }
    }
}
