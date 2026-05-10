package com.bong.client.entity;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

import java.util.EnumMap;
import java.util.Map;

public final class BongEntityRenderBootstrap {
    private static final Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> RENDERER_CLASSES =
        rendererClassMap();

    private BongEntityRenderBootstrap() {}

    public static void register() {
        BongEntityRegistry.register();
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.SPIRIT_NICHE), SpiritNicheRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.SPIRIT_EYE), SpiritEyeRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.RIFT_PORTAL), RiftPortalRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.FORGE_STATION), ForgeStationRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.ALCHEMY_FURNACE), AlchemyFurnaceRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.FORMATION_CORE), FormationCoreRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.LINGTIAN_PLOT), LingtianPlotRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.DRY_CORPSE), DryCorpseRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.BONE_SKELETON), BoneSkeletonRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.STORAGE_POUCH), StoragePouchRenderer::new);
        EntityRendererRegistry.register(BongEntityRegistry.type(BongEntityModelKind.STONE_CASKET), StoneCasketRenderer::new);
    }

    public static Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> rendererBindingsForTests() {
        return RENDERER_CLASSES;
    }

    private static Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> rendererClassMap() {
        EnumMap<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> map =
            new EnumMap<>(BongEntityModelKind.class);
        map.put(BongEntityModelKind.SPIRIT_NICHE, SpiritNicheRenderer.class);
        map.put(BongEntityModelKind.SPIRIT_EYE, SpiritEyeRenderer.class);
        map.put(BongEntityModelKind.RIFT_PORTAL, RiftPortalRenderer.class);
        map.put(BongEntityModelKind.FORGE_STATION, ForgeStationRenderer.class);
        map.put(BongEntityModelKind.ALCHEMY_FURNACE, AlchemyFurnaceRenderer.class);
        map.put(BongEntityModelKind.FORMATION_CORE, FormationCoreRenderer.class);
        map.put(BongEntityModelKind.LINGTIAN_PLOT, LingtianPlotRenderer.class);
        map.put(BongEntityModelKind.DRY_CORPSE, DryCorpseRenderer.class);
        map.put(BongEntityModelKind.BONE_SKELETON, BoneSkeletonRenderer.class);
        map.put(BongEntityModelKind.STORAGE_POUCH, StoragePouchRenderer.class);
        map.put(BongEntityModelKind.STONE_CASKET, StoneCasketRenderer.class);
        return Map.copyOf(map);
    }
}
