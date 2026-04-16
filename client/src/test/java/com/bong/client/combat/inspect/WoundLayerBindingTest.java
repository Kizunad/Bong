package com.bong.client.combat.inspect;

import com.bong.client.combat.store.WoundsStore;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.inventory.state.PhysicalBodyStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class WoundLayerBindingTest {
    @AfterEach void tearDown() {
        WoundsStore.resetForTests();
        PhysicalBodyStore.resetForTests();
    }

    @Test void severityMapsToWoundLevel() {
        WoundsStore.Wound w = new WoundsStore.Wound(
            "chest", "cut", 0.6f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L
        );
        assertEquals(WoundLevel.LACERATION, WoundLayerBinding.toWoundLevel(w));

        WoundsStore.Wound bf = new WoundsStore.Wound(
            "left_hand", "bone_fracture", 0.6f, WoundsStore.HealingState.STANCHED, 0f, false, 0L
        );
        assertEquals(WoundLevel.FRACTURE, WoundLayerBinding.toWoundLevel(bf));

        WoundsStore.Wound mild = new WoundsStore.Wound(
            "head", "cut", 0.1f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L
        );
        assertEquals(WoundLevel.BRUISE, WoundLayerBinding.toWoundLevel(mild));
    }

    @Test void buildBodyMapsChestWoundIntoPhysicalBody() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("chest", "cut", 0.6f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        PhysicalBody body = WoundLayerBinding.buildBody();
        assertEquals(WoundLevel.LACERATION, body.part(BodyPart.CHEST).wound());
        assertTrue(body.part(BodyPart.CHEST).bleedRate() > 0);
        assertEquals(WoundLevel.INTACT, body.part(BodyPart.HEAD).wound());
    }

    @Test void applyPushesToPhysicalBodyStore() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("head", "cut", 0.3f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        WoundLayerBinding.apply();
        assertNotNull(PhysicalBodyStore.snapshot());
        assertEquals(WoundLevel.ABRASION, PhysicalBodyStore.snapshot().part(BodyPart.HEAD).wound());
    }

    @Test void unknownPartIgnored() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("tail", "cut", 0.9f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        PhysicalBody body = WoundLayerBinding.buildBody();
        for (BodyPart bp : BodyPart.values()) {
            assertEquals(WoundLevel.INTACT, body.part(bp).wound());
        }
    }
}
