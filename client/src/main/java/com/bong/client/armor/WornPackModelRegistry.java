package com.bong.client.armor;

import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.Optional;

/**
 * plan-tarkov-backpack-v1 P4 — 穿戴背包件 template_id → 上身渲染资产（贴图 + 模型件）映射。
 *
 * <p>键集**镜像** {@code InventoryEquipRules.CONTAINER_TEMPLATE_IDS}（{@code worn_grass_pouch} /
 * {@code grass_pouch}）——客户端无 ItemRegistry，靠白名单识别 container_spec 件。新增可上身渲染的
 * 背包件时，本表与 {@code InventoryEquipRules} 白名单同步扩。前胸 front 变体（grass_pouch_front）
 * 留 P5，本阶段只有背面 BACK。
 *
 * <p>纯映射 / 无 GL 依赖，headless 可单测。实际 {@link net.minecraft.client.model.ModelPart} 由
 * {@link WornPackFeatureRenderer} 按 {@link WornPackModelKind} 懒加载烘焙。
 */
public final class WornPackModelRegistry {

    /** 挂点变体。本阶段仅 BACK（背面）；FRONT（前胸）留 P5。 */
    public enum WornPackModelKind {
        BACK
    }

    /**
     * 上身渲染规格：模板 id + 挂点变体 + entity 贴图。
     */
    public record WornPackModelSpec(String templateId, WornPackModelKind kind, Identifier textureId) {}

    private static final Identifier GRASS_POUCH_TEXTURE =
        new Identifier("bong", "textures/entity/grass_pouch.png");

    private static final Map<String, WornPackModelSpec> SPECS = Map.of(
        "worn_grass_pouch",
        new WornPackModelSpec("worn_grass_pouch", WornPackModelKind.BACK, GRASS_POUCH_TEXTURE),
        "grass_pouch",
        new WornPackModelSpec("grass_pouch", WornPackModelKind.BACK, GRASS_POUCH_TEXTURE)
    );

    private WornPackModelRegistry() {}

    /** 查模板对应的上身渲染规格；未注册（含 null/空串）返回 empty。 */
    public static Optional<WornPackModelSpec> get(String templateId) {
        if (templateId == null || templateId.isBlank()) {
            return Optional.empty();
        }
        return Optional.ofNullable(SPECS.get(templateId));
    }

    /** 已注册的可上身渲染背包件数量。 */
    public static int size() {
        return SPECS.size();
    }
}
