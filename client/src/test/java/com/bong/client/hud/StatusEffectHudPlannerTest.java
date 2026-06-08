package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatusEffectHudPlannerTest {
    @AfterEach void tearDown() { StatusEffectStore.resetForTests(); }

    @Test void emptyWhenNoEffects() {
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);
        assertTrue(cmds.isEmpty());
    }

    @Test void drawsSlotsForEachEffect() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("a", "A", StatusEffectStore.Kind.DOT, 1, 5_000, 0xFFFF0000, "", 0),
            new StatusEffectStore.Effect("b", "B", StatusEffectStore.Kind.BUFF, 3, 8_000, 0xFF00FF00, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);
        assertFalse(cmds.isEmpty());
        for (HudRenderCommand c : cmds) {
            assertEquals(HudRenderLayer.STATUS_EFFECTS, c.layer());
        }
        long stackText = cmds.stream().filter(HudRenderCommand::isText).count();
        // Second effect has stacks=3 → one text entry for ×3
        assertEquals(1L, stackText);
    }

    @Test void knownEffectRendersEmblemTextureNotTint() {
        // "bleeding" ships an emblem → expect a TEXTURED_RECT pointing at it,
        // and NO kind-tint fill rect occupying the inner icon area.
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("bleeding", "流血", StatusEffectStore.Kind.DOT, 1, 5_000, 0xFFE05050, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        String want = StatusEffectHudPlanner.ICON_BASE + "bleeding.png";
        assertTrue(cmds.stream().anyMatch(c -> c.isTexturedRect() && want.equals(c.texturePath())),
            "iconned effect 'bleeding' should draw emblem texture " + want
                + " but commands were " + cmds);
        // The 14×14 inner fill must be the texture, not a tint rect of the same size.
        assertFalse(cmds.stream().anyMatch(c ->
                c.isRect() && c.width() == StatusEffectHudPlanner.SLOT_SIZE - 4
                    && c.height() == StatusEffectHudPlanner.SLOT_SIZE - 4),
            "iconned effect must not also paint a kind-tint inner fill");
    }

    @Test void parameterizedIdResolvesToBaseEmblem() {
        // body_part_resist:<part> shares one emblem keyed on the colon-stripped base.
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("body_part_resist:head", "头部硬化", StatusEffectStore.Kind.BUFF, 1, 9_000, 0xFF55CC66, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        String want = StatusEffectHudPlanner.ICON_BASE + "body_part_resist.png";
        assertTrue(cmds.stream().anyMatch(c -> c.isTexturedRect() && want.equals(c.texturePath())),
            "parameterized id should map to base emblem " + want + " but commands were " + cmds);
    }

    @Test void unknownIdFallsBackToKindTint() {
        // An id with no shipped emblem must still fill the slot with a kind tint
        // (never a missing-texture draw, never a blank slot).
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("no_such_effect_zzz", "X", StatusEffectStore.Kind.DEBUFF, 1, 5_000, 0xFFFF8030, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        assertFalse(cmds.stream().anyMatch(HudRenderCommand::isTexturedRect),
            "un-iconned effect must NOT emit a texture command (would show missing-texture)");
        assertTrue(cmds.stream().anyMatch(c ->
                c.isRect() && c.width() == StatusEffectHudPlanner.SLOT_SIZE - 4
                    && c.height() == StatusEffectHudPlanner.SLOT_SIZE - 4),
            "un-iconned effect must fall back to a kind-tint inner fill");
    }

    @Test void nullOrEmptyIdFallsBackGracefully() {
        // StatusEffectStore.Effect coerces a null id to "" — buildCommands must
        // not crash and must fall back to a kind tint (never a texture draw).
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect(null, "空ID", StatusEffectStore.Kind.UNKNOWN, 1, 5_000, 0xFF808080, "", 0),
            new StatusEffectStore.Effect("", "空串", StatusEffectStore.Kind.BUFF, 1, 5_000, 0xFF55CC66, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        assertFalse(cmds.stream().anyMatch(HudRenderCommand::isTexturedRect),
            "null/empty id must not emit a TEXTURED_RECT (no '' icon ships)");
        long tintFills = cmds.stream().filter(c ->
            c.isRect() && c.width() == StatusEffectHudPlanner.SLOT_SIZE - 4
                && c.height() == StatusEffectHudPlanner.SLOT_SIZE - 4).count();
        assertEquals(2L, tintFills, "both blank-id effects must fall back to a kind-tint inner fill");
    }

    @Test void debuffRemainingBarUsesRedCountdown() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("stamina_crash", "体力虚脱", StatusEffectStore.Kind.DEBUFF, 1, 5_000, 0xFFFF8030, "", 0)
        ));

        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        assertTrue(cmds.stream().anyMatch(cmd ->
            cmd.isRect()
                && cmd.width() > 0
                && cmd.height() == 1
                && cmd.color() == StatusEffectHudPlanner.DEBUFF_REMAINING_BAR_COLOR
        ), "debuff countdown bar should be red");
    }
}
