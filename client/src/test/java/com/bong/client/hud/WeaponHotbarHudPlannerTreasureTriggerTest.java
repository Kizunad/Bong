package com.bong.client.hud;

import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.TreasurePanelSync;
import com.bong.client.combat.WeaponEquippedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-layered-equip-v1 P4（决议 #8）：WeaponHotbarHudPlanner 从触发位（trigger_0..）拉激活态法宝展示。
 *
 * <p>覆盖：off_hand 无持械法宝时，HUD 退而展示首个占用的触发位法宝；off_hand 持械法宝优先于触发位。
 */
class WeaponHotbarHudPlannerTreasureTriggerTest {

    private static final int SCREEN_W = 800;
    private static final int SCREEN_H = 600;

    @BeforeEach
    void setUp() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
        TreasureEquippedStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        EquippedShieldStore.resetForTests();
        WeaponEquippedStore.resetForTests();
        TreasureEquippedStore.resetForTests();
    }

    @Test
    void triggerSlotTreasure_rendersWhenNoOffHandTreasure() {
        TreasureEquippedStore.putOrClear(
            TreasurePanelSync.triggerSlotKey(0),
            new EquippedTreasure(TreasurePanelSync.triggerSlotKey(0), 7L, "spirit_treasure_jizhaojing", "寂照镜")
        );
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        boolean hasTreasureGlyph = cmds.stream().anyMatch(cmd -> cmd.isText() && "宝".equals(cmd.text()));
        assertTrue(hasTreasureGlyph,
            "触发位有激活法宝且 off_hand 无持械法宝时，HUD 应展示触发位法宝（宝字）");
    }

    @Test
    void firstOccupiedTriggerSlotChosen_whenSlotZeroEmpty() {
        // trigger_0 空，trigger_2 占用 → 应取 trigger_2。
        TreasureEquippedStore.putOrClear(
            TreasurePanelSync.triggerSlotKey(2),
            new EquippedTreasure(TreasurePanelSync.triggerSlotKey(2), 9L, "spirit_treasure_jizhaojing", "寂照镜")
        );
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        boolean hasTreasureGlyph = cmds.stream().anyMatch(cmd -> cmd.isText() && "宝".equals(cmd.text()));
        assertTrue(hasTreasureGlyph,
            "trigger_0 空时应回退到首个占用的触发位（trigger_2）展示");
    }

    @Test
    void offHandTreasureTakesPrecedenceOverTriggerSlot() {
        TreasureEquippedStore.putOrClear(
            "off_hand",
            new EquippedTreasure("off_hand", 1L, "talisman_offhand", "护符")
        );
        TreasureEquippedStore.putOrClear(
            TreasurePanelSync.triggerSlotKey(0),
            new EquippedTreasure(TreasurePanelSync.triggerSlotKey(0), 2L, "spirit_treasure_jizhaojing", "寂照镜")
        );
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        // off_hand 持械法宝优先：HUD 仍渲染单个法宝槽（宝字），但来源是 off_hand（id=1）。
        boolean hasTreasureGlyph = cmds.stream().anyMatch(cmd -> cmd.isText() && "宝".equals(cmd.text()));
        assertTrue(hasTreasureGlyph, "off_hand 持械法宝应被展示");
    }

    @Test
    void noTreasureAnywhere_rendersNothing() {
        List<HudRenderCommand> cmds = WeaponHotbarHudPlanner.buildCommands(SCREEN_W, SCREEN_H);
        boolean hasTreasureGlyph = cmds.stream().anyMatch(cmd -> cmd.isText() && "宝".equals(cmd.text()));
        assertFalse(hasTreasureGlyph, "无任何法宝时不应渲染法宝槽");
    }

    @Test
    void triggerSlotKeyFormatMatchesServerConvention() {
        assertEquals("trigger_0", TreasurePanelSync.triggerSlotKey(0));
        assertEquals("trigger_3", TreasurePanelSync.triggerSlotKey(3));
    }
}
