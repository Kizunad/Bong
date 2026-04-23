package com.bong.client.combat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

class WeaponEquippedStoreTest {

    @AfterEach
    void tearDown() {
        WeaponEquippedStore.resetForTests();
    }

    @Test
    void mainHandRenderWeaponFallsBackToTwoHand() {
        WeaponEquippedStore.putOrClear(
            "two_hand",
            new EquippedWeapon("two_hand", 1L, "wooden_staff", "staff", 80.0f, 100.0f, 0)
        );

        EquippedWeapon weapon = WeaponEquippedStore.mainHandRenderWeapon();

        assertNotNull(weapon);
        assertEquals("wooden_staff", weapon.templateId());
    }

    @Test
    void mainHandRenderWeaponPrefersMainHandOverTwoHand() {
        WeaponEquippedStore.putOrClear(
            "two_hand",
            new EquippedWeapon("two_hand", 1L, "wooden_staff", "staff", 80.0f, 100.0f, 0)
        );
        WeaponEquippedStore.putOrClear(
            "main_hand",
            new EquippedWeapon("main_hand", 2L, "iron_sword", "sword", 100.0f, 100.0f, 0)
        );

        assertEquals("iron_sword", WeaponEquippedStore.mainHandRenderWeapon().templateId());
    }
}
