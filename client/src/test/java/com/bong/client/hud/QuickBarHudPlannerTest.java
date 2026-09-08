package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QuickBarHudPlannerTest {
    @Test
    void bothRowsAreCentered() {
        SkillBarConfig skills = SkillBarConfig.empty();
        for (int screenWidth : new int[] {320, 961}) {
            List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
                QuickSlotConfig.empty(), skills, -1, CastState.idle(), List.of(), 0, screenWidth, 540);
            List<HudRenderCommand> frames = commands.stream()
                .filter(c -> c.isSvgRect() && c.layer() == HudRenderLayer.QUICK_BAR).toList();
            assertEquals(QuickSlotConfig.SLOT_COUNT + SkillBarConfig.SLOT_COUNT, frames.size());
            for (int rowY : frames.stream().mapToInt(HudRenderCommand::y).distinct().toArray()) {
                List<HudRenderCommand> row = frames.stream().filter(c -> c.y() == rowY).toList();
                int left = row.get(0).x();
                HudRenderCommand last = row.get(row.size() - 1);
                int right = last.x() + last.width();
                assertEquals(screenWidth / 2.0, (left + right) / 2.0, 0.5,
                    "两排必须各自居中，整数像素最多允许半像素舍入");
            }
        }
        // 扩至奇数格仍围绕同一中线，不能沿用旧两格的左边缘。
        for (int count : new int[] {2, 3}) {
            int left = QuickBarHudPlanner.rowLeftX(960, count);
            int lastRight = left + (count - 1) * (QuickBarHudPlanner.SLOT_SIZE + QuickBarHudPlanner.SLOT_GAP)
                + QuickBarHudPlanner.SLOT_SIZE;
            assertEquals(480, (left + lastRight) / 2);
        }
    }

    @Test
    void castHighlightsOnlyItsSourceRowAndCooldownStaysAboveTheIcon() {
        QuickSlotConfig quick = QuickSlotConfig.empty()
            .withSlot(0, new QuickSlotEntry("earth_crumb", "土块", 1000, 1000, ""))
            .withCooldownUntil(0, 2_000L);
        SkillBarConfig skills = SkillBarConfig.empty().withSlot(0,
            SkillBarEntry.skill("zhenmai.harden", "护脉", 1000, 1000, ""));
        for (CastState.Source source : CastState.Source.values()) {
            // slot 0 在上下两排同时绑定时，只能提示实际施法来源。
            List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
                quick, skills, -1, CastState.casting(source, 0, 1000, 0), List.of(), 500L, 960, 540);
            List<HudRenderCommand> frames = commands.stream()
                .filter(c -> c.isSvgRect() && c.layer() == HudRenderLayer.QUICK_BAR).toList();
            assertEquals(source == CastState.Source.QUICK_SLOT ? "selected" : "slot", frames.get(0).svgAssetKey());
            assertEquals(source == CastState.Source.SKILL_BAR ? "selected" : "slot",
                frames.get(QuickSlotConfig.SLOT_COUNT).svgAssetKey());
            HudRenderCommand icon = commands.stream().filter(HudRenderCommand::isItemTexture).findFirst().orElseThrow();
            HudRenderCommand cooldown = commands.stream().filter(c -> c.isRect()
                && c.color() == QuickBarHudPlanner.COOLDOWN_OVERLAY_COLOR).findFirst().orElseThrow();
            assertTrue(commands.indexOf(icon) > commands.indexOf(frames.get(0)), "槽框不能盖住图标");
            assertTrue(commands.indexOf(cooldown) > commands.indexOf(icon), "冷却遮罩必须覆盖图标");
        }
    }

    @Test
    void ringTracksProgressAndReturnsToEmptyAfterEitherTerminalState() {
        try {
            CastState cast = CastState.casting(0, 1000, 0);
            double earlier = visibleArcAmount(CastRingHudPlanner.buildCommands(cast, 250, 960, 540));
            double later = visibleArcAmount(CastRingHudPlanner.buildCommands(cast, 750, 960, 540));
            assertTrue(later > earlier, "进度增加时点亮弧段的总量必须增加");
            for (CastState terminal : List.of(cast.transitionToComplete(1000),
                cast.transitionToInterrupt(CastOutcome.USER_CANCEL, 1000))) {
                CastStateStore.replace(terminal);
                List<HudRenderCommand> commands = CastRingHudPlanner.buildCommands(CastStateStore.snapshot(), 1000, 960, 540);
                assertTrue(commands.stream().anyMatch(c -> c.svgAssetKey().equals(
                    terminal.phase() == CastState.Phase.COMPLETE ? "complete" : "interrupted")),
                    "完成与打断必须保留不同的可见结果");
                CastStateStore.tick(1300);
                assertTrue(CastRingHudPlanner.buildCommands(CastStateStore.snapshot(), 1300, 960, 540).isEmpty(),
                    "store 收尾后残环不得残留");
            }
        } finally {
            CastStateStore.resetForTests();
        }
    }

    private static double visibleArcAmount(List<HudRenderCommand> commands) {
        return commands.stream().filter(c -> CastRingHudPlanner.SEGMENT_KEYS.contains(c.svgAssetKey()))
            .mapToDouble(c -> (c.color() >>> 24) / 255.0).sum();
    }

    @Test
    void skillSlotUsesIconTextureWhenProvided() {
        SkillBarConfig skillBar = SkillBarConfig.of(
            new SkillBarEntry[] {
                SkillBarEntry.skill(
                    "zhenmai.parry",
                    "极限弹反",
                    50,
                    5000,
                    "bong-client:textures/gui/skill/zhenmai_parry.png"
                )
            },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            null,
            skillBar,
            0,
            null,
            List.of(),
            0L,
            320,
            240
        );

        HudRenderCommand icon = commands.stream()
            .filter(command -> command.isTexturedRect()
                && "bong-client:textures/gui/skill/zhenmai_parry.png".equals(command.texturePath()))
            .findFirst()
            .orElseThrow();
        assertEquals(QuickBarHudPlanner.SLOT_SIZE - 2 * QuickBarHudPlanner.ICON_INSET, icon.width());
        assertEquals(QuickBarHudPlanner.SLOT_SIZE - 2 * QuickBarHudPlanner.ICON_INSET, icon.height());
    }

    @Test
    void skillSlotFallsBackToTextWhenIconTextureMissing() {
        SkillBarConfig skillBar = SkillBarConfig.of(
            new SkillBarEntry[] {
                SkillBarEntry.skill("zhenmai.harden", "护脉", 250, 5000, "")
            },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            null,
            skillBar,
            0,
            null,
            List.of(),
            0L,
            320,
            240
        );

        assertTrue(commands.stream()
            .filter(HudRenderCommand::isText)
            .anyMatch(command -> "护脉".equals(command.text())));
    }

    @Test
    void zhenmaiSkillIconsExistAsClientResources() {
        List<String> icons = List.of(
            "zhenmai_parry",
            "zhenmai_neutralize",
            "zhenmai_multipoint",
            "zhenmai_harden",
            "zhenmai_sever_chain"
        );

        for (String icon : icons) {
            assertNotNull(
                QuickBarHudPlannerTest.class.getClassLoader()
                    .getResource("assets/bong-client/textures/gui/skill/" + icon + ".png"),
                "zhenmai skill icon resource should exist: " + icon
            );
        }
    }

    @Test
    void skillSlotUsesConfiguredIconTexture() {
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] {
                SkillBarEntry.skill(
                    "woliu.hold",
                    "持涡",
                    50,
                    500,
                    "bong:textures/gui/skill/woliu_hold.png"
                )
            },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            QuickSlotConfig.empty(),
            skills,
            0,
            CastState.idle(),
            List.of(),
            1_000L,
            960,
            540
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
            && cmd.texturePath().equals("bong:textures/gui/skill/woliu_hold.png")));
        assertFalse(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().equals("持")));
    }

    @Test
    void skillSlotWithMissingConfiguredIconFallsBackToTextWhenProbeIsProvided() {
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] {
                SkillBarEntry.skill(
                    "anqi.multi_shot",
                    "多发齐射",
                    50,
                    500,
                    "bong:textures/gui/skill/anqi_multi_shot.png"
                )
            },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            QuickSlotConfig.empty(),
            skills,
            0,
            CastState.idle(),
            List.of(),
            1_000L,
            960,
            540,
            path -> false
        );

        assertFalse(commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
            && cmd.texturePath().equals("bong:textures/gui/skill/anqi_multi_shot.png")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().equals("多")));
    }

    @Test
    void itemSlotUsesEntryIconTexture() {
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] {
                SkillBarEntry.item(
                    "earth_crumb",
                    "土块",
                    0,
                    0,
                    "bong-client:textures/gui/items/earth_crumb.png"
                )
            },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            QuickSlotConfig.empty(),
            skills,
            0,
            CastState.idle(),
            List.of(),
            1_000L,
            960,
            540
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
            && cmd.texturePath().equals("bong-client:textures/gui/items/earth_crumb.png")));
        assertFalse(commands.stream().anyMatch(cmd -> cmd.isItemTexture() && cmd.text().equals("earth_crumb")));
    }

    @Test
    void skillSlotFallsBackToShortTextWithoutIcon() {
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] { SkillBarEntry.skill("woliu.burst", "瞬涡", 50, 500, "") },
            new long[SkillBarConfig.SLOT_COUNT]
        );

        List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
            QuickSlotConfig.empty(),
            skills,
            0,
            CastState.idle(),
            List.of(),
            1_000L,
            960,
            540
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().equals("瞬涡")));
    }
}
