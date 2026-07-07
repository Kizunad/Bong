package com.bong.client.hud;

import com.bong.client.npc.NpcMoodStore;
import com.bong.client.npc.NpcMetadata;
import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.social.SocialStateStore;
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
        SocialStateStore.resetForTests();
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

        assertFalse(
            state.isEmpty(),
            "expected non-empty target because NPC metadata is present, actual empty"
        );
        assertEquals(
            TargetInfoState.Kind.NPC,
            state.kind(),
            "expected kind NPC because metadata target represents an NPC, actual: " + state.kind()
        );
        assertEquals(
            "entity:126",
            state.targetId(),
            "expected target id entity:126 because metadata entity id is 126, actual: " + state.targetId()
        );
        assertEquals(
            "噬灵鼠",
            state.displayName(),
            "expected display name from NPC metadata, actual: " + state.displayName()
        );
        assertEquals(
            "Awaken",
            state.realm(),
            "expected realm Awaken because metadata realm is Awaken, actual: " + state.realm()
        );
        assertEquals(
            0.37,
            state.hpRatio(),
            "expected hp ratio 0.37 because metadata supplies it, actual: " + state.hpRatio()
        );
        assertEquals(
            0.22,
            state.qiRatio(),
            "expected qi ratio 0.22 because metadata supplies it, actual: " + state.qiRatio()
        );
    }

    @Test
    void anonymousPlayerTargetInfoUsesSocialAnonymityPlaceholder() {
        SocialStateStore.replaceAnonymity("char:viewer", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                "offline:LeakedName:char-uuid",
                true,
                "LeakedName",
                "Awaken",
                "",
                List.of()
            )
        ));

        String displayName = TargetInfoState.playerDisplayNameForTargetInfo(
            "offline:LeakedName:char-uuid",
            "LeakedName",
            "LeakedName"
        );
        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.PLAYER,
            "entity:7",
            displayName,
            "",
            0.8,
            0.0,
            1_000L
        );

        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);

        assertEquals(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME, state.displayName());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME)));
        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().contains("LeakedName")));
    }

    @Test
    void exposedPlayerTargetInfoKeepsKnownName() {
        SocialStateStore.replaceAnonymity("char:viewer", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                "offline:KnownAlly:char-uuid",
                false,
                "KnownAlly",
                "Awaken",
                "",
                List.of()
            )
        ));

        String displayName = TargetInfoState.playerDisplayNameForTargetInfo(
            "offline:KnownAlly:char-uuid",
            "KnownAlly",
            "KnownAlly"
        );
        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.PLAYER,
            "entity:8",
            displayName,
            "",
            0.8,
            0.0,
            1_000L
        );

        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);

        assertEquals("KnownAlly", state.displayName());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("KnownAlly")));
        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().contains(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME)));
    }

    @Test
    void pseudoPlayerEntityTargetInfoKeepsNonProfileDisplayName() {
        String displayName = TargetInfoState.playerDisplayNameForTargetInfo(
            "00000000-0000-0000-0000-000000000000",
            "Remains_1234abcd",
            "遗骸"
        );
        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.PLAYER,
            "entity:9",
            displayName,
            "",
            0.0,
            0.0,
            1_000L
        );

        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);

        assertEquals("遗骸", state.displayName());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("遗骸")));
        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().contains(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME)));
        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().contains("Remains_1234abcd")));
    }

    @Test
    void unknownPlayerDecoratedDisplayNameStillUsesAnonymousPlaceholder() {
        String displayName = TargetInfoState.playerDisplayNameForTargetInfo(
            "offline:LeakedName:char-uuid",
            "LeakedName",
            "[队伍] LeakedName"
        );

        TargetInfoState state = TargetInfoState.create(
            TargetInfoState.Kind.PLAYER,
            "entity:10",
            displayName,
            "",
            0.8,
            0.0,
            1_000L
        );
        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(state, 1_500L, FIXED_WIDTH, 320, 180);

        assertEquals(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME, state.displayName());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains(TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME)));
        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().contains("LeakedName")));
    }
}
