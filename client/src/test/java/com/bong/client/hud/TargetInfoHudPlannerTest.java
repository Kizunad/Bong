package com.bong.client.hud;

import com.bong.client.npc.NpcMoodStore;
import com.bong.client.npc.NpcMetadata;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TargetInfoHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @AfterEach
    void reset() {
        PlayerStateStore.resetForTests();
        NpcMoodStore.clearAll();
        NpcMetadataStore.clearAll();
    }

    @Test
    void targetInfoShowsOnAttackAndExpiresAfterFiveSeconds() {
        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Solidify",
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
        ));
        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.NPC,
            "npc:mantis",
            "刀螳",
            "Condense",
            0.42,
            0.33,
            1_000L
        );

        List<HudRenderCommand> visible = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);
        List<HudRenderCommand> expired = TargetInfoHudPlanner.buildCommands(state, 6_000L, FIXED_WIDTH, 320, 180);

        assertFalse(visible.isEmpty());
        assertTrue(visible.stream().anyMatch(cmd -> cmd.text().contains("刀螳")));
        assertTrue(visible.stream().anyMatch(cmd -> cmd.text().contains("凝脉")));
        assertTrue(expired.isEmpty());
    }

    @Test
    void strongerNpcRealmIsHidden() {
        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Induce",
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
        ));
        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.NPC,
            "npc:mantis",
            "刀螳",
            "Condense",
            0.42,
            0.33,
            1_000L
        );

        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);

        assertTrue(commands.stream().anyMatch(cmd -> "???".equals(cmd.text())));
        assertTrue(commands.stream().noneMatch(
            cmd -> (cmd.color() & 0x00FFFFFF) == (TargetInfoHudPlanner.QI_COLOR & 0x00FFFFFF)
        ));
    }

    @Test
    void npcMetadataBuildsTargetInfoWithoutLivingEntityHealth() {
        NpcMetadata metadata = new NpcMetadata(
            126,
            "beast",
            "Awaken",
            "",
            "",
            -80,
            "噬灵鼠",
            "",
            "",
            "",
            0.37,
            0.22
        );

        TargetInfoState state = TargetInfoState.fromNpcMetadata(metadata, 7_000L);

        assertFalse(state.isEmpty());
        assertEquals(TargetInfoState.Kind.NPC, state.kind());
        assertEquals("entity:126", state.targetId());
        assertEquals("噬灵鼠", state.displayName());
        assertEquals("Awaken", state.realm());
        assertEquals(0.37, state.hpRatio());
        assertEquals(0.22, state.qiRatio());
    }
}
