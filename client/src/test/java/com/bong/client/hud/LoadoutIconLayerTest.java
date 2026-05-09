package com.bong.client.hud;

import com.bong.client.combat.SkillBarEntry;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LoadoutIconLayerTest {
    @Test
    void skillWithIconBuildsTextureAndAnqiOverlay() {
        SkillBarEntry entry = SkillBarEntry.skill(
            "anqi.multi_shot",
            "Multi Shot",
            0,
            0,
            "bong-client:textures/gui/skills/anqi_multi_shot.png"
        );

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertEquals(3, commands.size());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
            && "bong-client:textures/gui/skills/anqi_multi_shot.png".equals(cmd.texturePath())));
        assertEquals(2, commands.stream().filter(HudRenderCommand::isRect).count());
    }

    @Test
    void echoFractalAddsSecondaryRectMarker() {
        SkillBarEntry entry = SkillBarEntry.skill(
            "anqi.echo_fractal",
            "Echo Fractal",
            0,
            0,
            "bong-client:textures/gui/skills/anqi_echo_fractal.png"
        );

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertEquals(4, commands.size());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isRect()
            && cmd.x() == 20
            && cmd.y() == 22
            && cmd.width() == 4
            && cmd.height() == 4));
    }

    @Test
    void skillWithoutIconKeepsQuickBarTextFallbackAvailable() {
        SkillBarEntry entry = SkillBarEntry.skill("anqi.soul_inject", "Soul Inject", 0, 0, "");

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertTrue(commands.isEmpty());
    }

    @Test
    void nullEntryKeepsQuickBarTextFallbackAvailable() {
        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(null, 10, 20, 16);

        assertTrue(commands.isEmpty());
    }
}
