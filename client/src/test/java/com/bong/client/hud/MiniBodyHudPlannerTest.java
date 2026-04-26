package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MiniBodyHudPlannerTest {

    @Test
    void inactiveStateRendersNothing() {
        List<HudRenderCommand> cmds = MiniBodyHudPlanner.buildCommands(
            CombatHudState.empty(), null, null, 0L, 1920, 1080);
        assertTrue(cmds.isEmpty(), "inactive CombatHudState must not emit any commands");
    }

    @Test
    void zeroViewportRendersNothing() {
        CombatHudState hud = CombatHudState.create(1.0f, 0.8f, 0.6f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 0, 0);
        assertTrue(cmds.isEmpty(), "0-sized viewport must emit nothing (avoids off-screen rects)");
    }

    @Test
    void activeStateEmitsPanelSilhouetteAndBars() {
        CombatHudState hud = CombatHudState.create(1.0f, 0.80f, 0.40f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080);

        assertFalse(cmds.isEmpty());
        // Every command must be a rect and live on the MINI_BODY layer.
        for (HudRenderCommand c : cmds) {
            assertTrue(c.isRect(), "expected all rect: " + c.kind());
            assertEquals(HudRenderLayer.MINI_BODY, c.layer());
        }
    }

    @Test
    void woundDotsAreRenderedForNonIntactParts() {
        CombatHudState hud = CombatHudState.create(0.9f, 0.5f, 0.5f, DerivedAttrFlags.none());
        PhysicalBody body = PhysicalBody.builder()
            .wound(BodyPart.CHEST, WoundLevel.LACERATION)
            .wound(BodyPart.LEFT_CALF, WoundLevel.FRACTURE)
            .build();

        int noBodyCount = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).size();
        int withBodyCount = MiniBodyHudPlanner.buildCommands(hud, body, null, 0L, 1920, 1080).size();

        assertEquals(noBodyCount + 2, withBodyCount,
            "two wounds should add exactly two rect commands");
    }

    @Test
    void lowBarFlashesBorderOnOddHalfSecond() {
        CombatHudState hud = CombatHudState.create(1.0f, 0.10f, 1.0f, DerivedAttrFlags.none());
        List<HudRenderCommand> dim = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080);
        List<HudRenderCommand> bright = MiniBodyHudPlanner.buildCommands(hud, null, null, 500L, 1920, 1080);
        // The 4-edge flash adds 4 rects when nowMillis/500 is even.
        assertTrue(dim.size() > bright.size(),
            "blink phase 0 should have 4 extra rects vs phase 1 (off)");
    }

    @Test
    void qiBarFillsProportionally() {
        CombatHudState full = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none());
        CombatHudState mid = CombatHudState.create(1.0f, 0.5f, 1.0f, DerivedAttrFlags.none());
        CombatHudState empty = CombatHudState.create(1.0f, 0.0f, 1.0f, DerivedAttrFlags.none());

        // Use the blink-off phase (500ms) so low-threshold border rects don't skew the count.
        long blinkOff = 500L;
        int fullSize = MiniBodyHudPlanner.buildCommands(full, null, null, blinkOff, 1920, 1080).size();
        int midSize = MiniBodyHudPlanner.buildCommands(mid, null, null, blinkOff, 1920, 1080).size();
        int emptySize = MiniBodyHudPlanner.buildCommands(empty, null, null, blinkOff, 1920, 1080).size();

        // When qi is 0%, the qi fill rect is omitted (one less command).
        assertTrue(emptySize < midSize, "empty qi skips fill rect: empty=" + emptySize + " mid=" + midSize);
        assertEquals(fullSize, midSize, "non-zero qi always emits the fill rect");
    }

    @Test
    void brokenArmorAddsCrackGlyphs() {
        CombatHudState hud = CombatHudState.create(1.0f, 0.8f, 0.6f, DerivedAttrFlags.none());

        var equipped = new EnumMap<EquipSlotType, InventoryItem>(EquipSlotType.class);
        equipped.put(
            EquipSlotType.CHEST,
            InventoryItem.createFull(1L, "fake_spirit_hide", "伪灵皮", 2, 2, 3.0, "common", "", 1, 1.0, 0.0)
        );

        int base = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).size();
        int withCrack = MiniBodyHudPlanner.buildCommands(hud, null, equipped, 0L, 1920, 1080).size();

        // Chest crack glyph draws 9 tiny rects per covered part (CHEST + ABDOMEN).
        assertEquals(base + 18, withCrack);
    }
}
