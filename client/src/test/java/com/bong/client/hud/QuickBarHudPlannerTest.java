package com.bong.client.hud;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QuickBarHudPlannerTest {
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
}
