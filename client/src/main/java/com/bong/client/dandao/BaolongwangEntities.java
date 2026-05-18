package com.bong.client.dandao;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-dandao-path-v1 P4 -- EntityType registry for Baolongwang BOSS.
 *
 * <p>Raw ID follows after fauna (126..=145) + BongEntityRegistry (146..=159).
 * Baolongwang gets raw_id 160. Registration order in BongClient matters.
 */
public final class BaolongwangEntities {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/dandao/baolongwang");
    public static final Identifier BAOLONGWANG_ID = new Identifier("bong", "baolongwang");

    /**
     * Expected raw id -- after BongEntityRegistry (146..=159).
     * Fauna ends at 145, BongEntityModelKind has 14 entries (146..=159),
     * so baolongwang = 160.
     */
    public static final int EXPECTED_RAW_ID = 160;

    private BaolongwangEntities() {}

    public static EntityType<BaolongwangEntity> baolongwang() {
        return Holder.BAOLONGWANG;
    }

    public static void register() {
        EntityType<BaolongwangEntity> type = baolongwang();
        int rawId = Registries.ENTITY_TYPE.getRawId(type);
        if (rawId != EXPECTED_RAW_ID) {
            LOGGER.error(
                "[bong][dandao] raw_id MISMATCH: {} expected {}, got {}. "
                    + "Check BongClient entity registration order.",
                BAOLONGWANG_ID,
                EXPECTED_RAW_ID,
                rawId
            );
            return;
        }
        LOGGER.info(
            "[bong][dandao] registered EntityType {} raw_id={}",
            BAOLONGWANG_ID,
            rawId
        );
    }

    private static final class Holder {
        // 4x4x6 block body. Large tracking range for BOSS visibility.
        private static final EntityType<BaolongwangEntity> BAOLONGWANG = Registry.register(
            Registries.ENTITY_TYPE,
            BAOLONGWANG_ID,
            EntityType.Builder
                .create(BaolongwangEntity::new, SpawnGroup.MONSTER)
                .setDimensions(4.0f, 6.0f)
                .maxTrackingRange(128)
                .trackingTickInterval(3)
                .disableSaving()
                .disableSummon()
                .build(BAOLONGWANG_ID.toString())
        );
    }
}
