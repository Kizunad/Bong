package com.bong.client.whale;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * EntityType registry holder for {@code bong:whale}.
 *
 * <p>Phase B-1：仅在客户端注册，配 /whale-debug 本地生成做渲染验证。
 * Phase B-2 server 端 Valence 自定义 EntityKind 接入时，必须保证 server
 * 用的协议数值 ID 与本注册的 raw id 对齐（Fabric registry 按注册顺序
 * 分配，所以 BongClient 调用顺序敏感）。
 */
public final class WhaleEntities {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/whale");
    public static final Identifier WHALE_ID = new Identifier("bong", "whale");

    /**
     * 协议契约 raw id —— 与 server 的 {@code WHALE_ENTITY_KIND::new(133)} 对齐。
     *
     * <p>Fabric 注册顺序敏感：当前打包 mod 栈会先注册 8 个非 Bong custom entity；
     * BongClient 再注册 botany_plant_v2 (raw_id=132) → whale (133) → fauna (134..=141)。
     * 后续 plan-entity-model-v1 的 raw id 由 BongEntityRegistry 的 EntityType 注册顺序决定
     * (142..=152)，BongEntityRenderBootstrap 只绑定 renderer。任何新 Bong EntityType 插队都会让
     * server 端 {@code WHALE_ENTITY_KIND} / {@code BONG_*_ENTITY_KIND} 偏移。
     */
    public static final int EXPECTED_RAW_ID = 133;

    private WhaleEntities() {}

    public static EntityType<WhaleEntity> whale() {
        return Holder.WHALE;
    }

    public static void register() {
        EntityType<WhaleEntity> type = whale();
        int rawId = Registries.ENTITY_TYPE.getRawId(type);
        if (rawId != EXPECTED_RAW_ID) {
            // 不再 throw —— mod 加载顺序差异在不同 modpack 都可能让 raw id 偏移，
            // 把 client 整个干掉太重；改成显著 ERROR 日志，让 whale 自己渲染错位
            // （或不渲染），其余 mod 仍可玩。运维或修 mod 顺序即可恢复对齐。
            LOGGER.error(
                "[bong][whale] raw_id MISMATCH：{} 期望 {}，实际 {}。"
                    + "server 的 WHALE_ENTITY_KIND 不再与本端对齐 —— 鲸将无法正确渲染/动作。"
                    + "修复方法：检查 BongClient onInitializeClient 中 EntityType 注册顺序，"
                    + "新 EntityType 必须排在 WhaleRenderBootstrap.register() 之后；"
                    + "或调整 server WHALE_ENTITY_KIND 与本端 raw_id 一致。",
                WHALE_ID,
                EXPECTED_RAW_ID,
                rawId
            );
            return;
        }
        LOGGER.info(
            "[bong][whale] registered EntityType {} raw_id={} (matches server's WHALE_ENTITY_KIND)",
            WHALE_ID,
            rawId
        );
    }

    private static final class Holder {
        // 视觉边界 ~5.4×2.3×8.6 块（geo.json 内坐标 X86×Y37×Z137）。dimensions 给
        // 略大点的 hitbox 防 frustum cull。trackingRange 加大到 256 因为这是空中
        // 大体型，远距离才进视野。
        private static final EntityType<WhaleEntity> WHALE = Registry.register(
            Registries.ENTITY_TYPE,
            WHALE_ID,
            EntityType.Builder
                .create(WhaleEntity::new, SpawnGroup.MISC)
                .setDimensions(9.0f, 3.0f)
                .maxTrackingRange(256)
                .trackingTickInterval(3)
                .disableSaving()
                .disableSummon()
                .build(WHALE_ID.toString())
        );
    }
}
