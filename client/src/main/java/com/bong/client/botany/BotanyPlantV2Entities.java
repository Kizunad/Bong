package com.bong.client.botany;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;

public final class BotanyPlantV2Entities {
    public static final Identifier BOTANY_PLANT_V2_ID = new Identifier("bong", "botany_plant_v2");

    private BotanyPlantV2Entities() {}

    public static EntityType<BotanyPlantV2Entity> botanyPlantV2() {
        return Holder.BOTANY_PLANT_V2;
    }

    public static void register() {
        botanyPlantV2();
    }

    private static final class Holder {
        private static final EntityType<BotanyPlantV2Entity> BOTANY_PLANT_V2 = Registry.register(
            Registries.ENTITY_TYPE,
            BOTANY_PLANT_V2_ID,
            EntityType.Builder
                .create(BotanyPlantV2Entity::new, SpawnGroup.MISC)
                .setDimensions(0.6f, 0.9f)
                .maxTrackingRange(64)
                .trackingTickInterval(10)
                .disableSaving()
                .disableSummon()
                .build(BOTANY_PLANT_V2_ID.toString())
        );
    }
}
