package com.bong.client.fauna;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import net.minecraft.world.World;

import java.util.EnumMap;
import java.util.Map;

public final class FaunaEntities {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/fauna");

    private FaunaEntities() {
    }

    public static EntityType<FaunaEntity> type(FaunaVisualKind kind) {
        return Holder.TYPES.get(kind);
    }

    public static void register() {
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            EntityType<FaunaEntity> type = type(kind);
            int rawId = Registries.ENTITY_TYPE.getRawId(type);
            if (rawId != kind.expectedRawId()) {
                LOGGER.error(
                    "[bong][fauna] raw_id MISMATCH: {} expected {}, got {}. "
                        + "Fauna custom entities must be registered immediately after whale.",
                    kind.entityId(),
                    kind.expectedRawId(),
                    rawId
                );
            } else {
                LOGGER.info("[bong][fauna] registered {} raw_id={}", kind.entityId(), rawId);
            }
        }
    }

    private static EntityType<FaunaEntity> build(FaunaVisualKind kind) {
        return Registry.register(
            Registries.ENTITY_TYPE,
            kind.entityId(),
            EntityType.Builder
                .<FaunaEntity>create(
                    (EntityType<FaunaEntity> type, World world) -> new FaunaEntity(type, world, kind),
                    SpawnGroup.MONSTER
                )
                .setDimensions(kind.dimensions().width, kind.dimensions().height)
                .maxTrackingRange(96)
                .trackingTickInterval(3)
                .disableSaving()
                .disableSummon()
                .build(kind.entityId().toString())
        );
    }

    private static final class Holder {
        private static final Map<FaunaVisualKind, EntityType<FaunaEntity>> TYPES = buildAll();

        private static Map<FaunaVisualKind, EntityType<FaunaEntity>> buildAll() {
            EnumMap<FaunaVisualKind, EntityType<FaunaEntity>> types = new EnumMap<>(FaunaVisualKind.class);
            for (FaunaVisualKind kind : FaunaVisualKind.values()) {
                types.put(kind, build(kind));
            }
            return Map.copyOf(types);
        }
    }
}
