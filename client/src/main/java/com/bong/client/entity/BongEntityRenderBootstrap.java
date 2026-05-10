package com.bong.client.entity;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;
import net.minecraft.client.render.entity.EntityRendererFactory;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

public final class BongEntityRenderBootstrap {
    private record RendererBinding(
        BongEntityModelKind kind,
        Class<? extends BongModeledEntityRenderer> rendererClass,
        EntityRendererFactory<BongModeledEntity> factory
    ) {
    }

    private static final List<RendererBinding> BINDINGS = List.of(
        new RendererBinding(BongEntityModelKind.SPIRIT_NICHE, SpiritNicheRenderer.class, SpiritNicheRenderer::new),
        new RendererBinding(BongEntityModelKind.SPIRIT_EYE, SpiritEyeRenderer.class, SpiritEyeRenderer::new),
        new RendererBinding(BongEntityModelKind.RIFT_PORTAL, RiftPortalRenderer.class, RiftPortalRenderer::new),
        new RendererBinding(BongEntityModelKind.FORGE_STATION, ForgeStationRenderer.class, ForgeStationRenderer::new),
        new RendererBinding(BongEntityModelKind.ALCHEMY_FURNACE, AlchemyFurnaceRenderer.class, AlchemyFurnaceRenderer::new),
        new RendererBinding(BongEntityModelKind.FORMATION_CORE, FormationCoreRenderer.class, FormationCoreRenderer::new),
        new RendererBinding(BongEntityModelKind.LINGTIAN_PLOT, LingtianPlotRenderer.class, LingtianPlotRenderer::new),
        new RendererBinding(BongEntityModelKind.DRY_CORPSE, DryCorpseRenderer.class, DryCorpseRenderer::new),
        new RendererBinding(BongEntityModelKind.BONE_SKELETON, BoneSkeletonRenderer.class, BoneSkeletonRenderer::new),
        new RendererBinding(BongEntityModelKind.STORAGE_POUCH, StoragePouchRenderer.class, StoragePouchRenderer::new),
        new RendererBinding(BongEntityModelKind.STONE_CASKET, StoneCasketRenderer.class, StoneCasketRenderer::new)
    );

    private static final Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> RENDERER_CLASSES =
        rendererClassMap();

    private BongEntityRenderBootstrap() {}

    public static void register() {
        LingtianPlotBlock.register();
        BongEntityRegistry.register();
        for (RendererBinding binding : BINDINGS) {
            EntityRendererRegistry.register(BongEntityRegistry.type(binding.kind()), binding.factory());
        }
    }

    public static Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> rendererBindingsForTests() {
        return RENDERER_CLASSES;
    }

    private static Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> rendererClassMap() {
        EnumMap<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> map =
            new EnumMap<>(BongEntityModelKind.class);
        for (RendererBinding binding : BINDINGS) {
            map.put(binding.kind(), binding.rendererClass());
        }
        return Map.copyOf(map);
    }
}
