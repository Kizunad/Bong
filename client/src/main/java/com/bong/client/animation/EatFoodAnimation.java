package com.bong.client.animation;

import net.minecraft.util.Identifier;

public final class EatFoodAnimation {
    public static final Identifier ID = BongAnimations.EAT_FOOD;
    public static final int PRIORITY = 200;
    public static final int DURATION_TICKS = 40;

    private EatFoodAnimation() {
    }

    public static boolean isEatFood(Identifier id) {
        return ID.equals(id);
    }
}
