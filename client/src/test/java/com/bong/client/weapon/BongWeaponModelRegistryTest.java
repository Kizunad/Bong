package com.bong.client.weapon;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class BongWeaponModelRegistryTest {
    @Test
    void registryExposesIndependentModelPathsForFormerPlaceholderWeapons() {
        BongWeaponModelRegistry.Entry ironSword = BongWeaponModelRegistry.get("iron_sword").orElseThrow();
        BongWeaponModelRegistry.Entry rustedBlade = BongWeaponModelRegistry.get("rusted_blade").orElseThrow();
        BongWeaponModelRegistry.Entry woodenStaff = BongWeaponModelRegistry.get("wooden_staff").orElseThrow();
        BongWeaponModelRegistry.Entry spiritSword = BongWeaponModelRegistry.get("spirit_sword").orElseThrow();

        assertEquals("bong:models/item/iron_sword/iron_sword.obj", ironSword.bongObjModelPath());
        assertEquals("bong:models/item/rusted_blade/rusted_blade.obj", rustedBlade.bongObjModelPath());
        assertEquals("bong:models/item/wooden_staff/wooden_staff.obj", woodenStaff.bongObjModelPath());
        assertEquals("bong:models/item/spirit_sword/spirit_sword.obj", spiritSword.bongObjModelPath());

        assertFalse(ironSword.bongObjModelPath().contains("placeholder"));
        assertFalse(woodenStaff.bongObjModelPath().contains("wooden_totem"));
        assertFalse(spiritSword.bongObjModelPath().contains("flying_sword_feixuan"));
    }

    @Test
    void registryUsesDistinctVanillaHostsForSwordFamilyEntries() {
        assertEquals("item/iron_sword", BongWeaponModelRegistry.get("iron_sword").orElseThrow().vanillaModelPath());
        assertEquals("item/netherite_sword", BongWeaponModelRegistry.get("rusted_blade").orElseThrow().vanillaModelPath());
        assertEquals("item/nether_star", BongWeaponModelRegistry.get("spirit_sword").orElseThrow().vanillaModelPath());
        assertEquals("item/diamond_sword", BongWeaponModelRegistry.get("flying_sword_feixuan").orElseThrow().vanillaModelPath());
    }
}
