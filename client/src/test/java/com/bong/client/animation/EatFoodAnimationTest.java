package com.bong.client.animation;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class EatFoodAnimationTest {
    @Test
    void exposesCanonicalAnimationIdAndPriority() {
        assertEquals("bong", EatFoodAnimation.ID.getNamespace());
        assertEquals("eat_food", EatFoodAnimation.ID.getPath());
        assertEquals(200, EatFoodAnimation.PRIORITY);
        assertEquals(40, EatFoodAnimation.DURATION_TICKS);
        assertTrue(EatFoodAnimation.isEatFood(BongAnimations.EAT_FOOD));
    }

    @Test
    void packagedJsonAnimationExists() {
        ClassLoader loader = Thread.currentThread().getContextClassLoader();
        assertNotNull(loader.getResource("assets/bong/player_animation/eat_food.json"));
    }
}
