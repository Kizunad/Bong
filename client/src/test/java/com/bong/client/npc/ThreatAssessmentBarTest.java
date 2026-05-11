package com.bong.client.npc;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertEquals;

class ThreatAssessmentBarTest {
    private static final PlayerStateViewModel AWAKEN = player("Awaken");
    private static final PlayerStateViewModel CONDENSE = player("Condense");
    private static final PlayerStateViewModel SPIRIT = player("Spirit");

    @Test
    void threat_bar_hidden_below_ningmai() {
        NpcMoodState mood = new NpcMoodState(42, "alert", 0.5, null, null, 1_000L);

        assertFalse(ThreatAssessmentBar.visibleFor(AWAKEN, mood));
        assertTrue(ThreatAssessmentBar.visibleFor(CONDENSE, mood));
    }

    @Test
    void threat_bar_color_segments() {
        assertEquals(ThreatAssessmentBar.COLOR_LOW, ThreatAssessmentBar.colorFor(0.10));
        assertEquals(ThreatAssessmentBar.COLOR_MID, ThreatAssessmentBar.colorFor(0.45));
        assertEquals(ThreatAssessmentBar.COLOR_HIGH, ThreatAssessmentBar.colorFor(0.80));
        assertEquals("已癫狂", ThreatAssessmentBar.labelFor(0.95));
    }

    @Test
    void flip_shatter_animation() {
        NpcMoodState mood = new NpcMoodState(42, "hostile", 0.95, "高", "此人真元快空了，动手！", 1_000L);

        List<HudRenderCommand> commands = ThreatAssessmentBar.buildCommands(mood, SPIRIT, 10, 10, 255, text -> text.length() * 6);

        assertTrue(commands.stream().anyMatch(cmd -> "已癫狂".equals(cmd.text())));
        assertTrue(commands.size() >= 7, "high threat should add shatter fragments and sense text");
    }

    @Test
    void reputation_indicator_in_inspect() {
        assertEquals("信任", NpcReputationIndicator.labelFor(70));
        assertEquals("敌视", NpcReputationIndicator.labelFor(-80));
        assertTrue(NpcReputationIndicator.fillWidth(80) > NpcReputationIndicator.fillWidth(-80));
    }

    private static PlayerStateViewModel player(String realm) {
        return PlayerStateViewModel.create(
            realm,
            "offline:Azure",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "qingyun",
            "青云断峰",
            0.7
        );
    }
}
