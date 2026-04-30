package com.bong.client.botany;

import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.data.DataTracker;
import net.minecraft.entity.data.TrackedData;
import net.minecraft.entity.data.TrackedDataHandlerRegistry;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.world.World;

/** Single client-side carrier entity for all v2 botany plants. */
public final class BotanyPlantV2Entity extends Entity {
    private static final TrackedData<String> PLANT_ID = DataTracker.registerData(
        BotanyPlantV2Entity.class,
        TrackedDataHandlerRegistry.STRING
    );

    public BotanyPlantV2Entity(EntityType<? extends BotanyPlantV2Entity> type, World world) {
        super(type, world);
        this.noClip = true;
    }

    public String plantId() {
        return dataTracker.get(PLANT_ID);
    }

    public void setPlantId(String plantId) {
        dataTracker.set(PLANT_ID, plantId == null ? "" : plantId.trim());
    }

    @Override
    protected void initDataTracker() {
        dataTracker.startTracking(PLANT_ID, "");
    }

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {
        setPlantId(nbt.getString("PlantId"));
    }

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {
        nbt.putString("PlantId", plantId());
    }
}
