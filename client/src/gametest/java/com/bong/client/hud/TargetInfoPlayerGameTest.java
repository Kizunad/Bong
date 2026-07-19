package com.bong.client.hud;

import com.bong.client.npc.NpcMetadataStore;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.social.SocialStateStore;
import com.bong.client.state.PlayerStateStore;
import net.fabricmc.fabric.api.gametest.v1.FabricGameTest;
import net.minecraft.scoreboard.Team;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.test.GameTest;
import net.minecraft.test.TestContext;
import net.minecraft.text.Text;

import java.util.List;

public final class TargetInfoPlayerGameTest implements FabricGameTest {
    private static final long OBSERVED_AT_MILLIS = 1_000L;
    private static final long RENDER_AT_MILLIS = 1_500L;
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH =
        text -> text == null ? 0 : text.length() * 6;

    @GameTest(templateName = FabricGameTest.EMPTY_STRUCTURE)
    public void anonymousPlayerHidesProfileAndDecoratedName(TestContext context) {
        resetStores();
        ObservedPlayer observed = createObservedPlayer(context, "anonymous-target", "[匿名队] ");
        SocialStateStore.replaceAnonymity("char:viewer", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                observed.player().getUuidAsString(),
                true,
                observed.profileName(),
                "Awaken",
                "",
                List.of()
            )
        ));

        ObservedTarget target = observeAndRender(observed.player());

        assertPlayerSnapshot(context, target.state(), observed.player());
        assertEquals(
            context,
            TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME,
            target.state().displayName(),
            "anonymous player snapshot must use the anonymous placeholder"
        );
        context.assertTrue(
            containsText(target.commands(), TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME),
            "anonymous player HUD must render the anonymous placeholder"
        );
        context.assertFalse(
            containsText(target.commands(), observed.profileName()),
            "anonymous player HUD must not leak the real profile name: " + observed.profileName()
        );
        context.assertFalse(
            containsText(target.commands(), observed.decoratedName()),
            "anonymous player HUD must not leak the scoreboard-decorated name: " + observed.decoratedName()
        );
        context.complete();
    }

    @GameTest(templateName = FabricGameTest.EMPTY_STRUCTURE)
    public void exposedPlayerKeepsScoreboardDecoratedName(TestContext context) {
        resetStores();
        ObservedPlayer observed = createObservedPlayer(context, "exposed-target", "[同盟] ");
        SocialStateStore.replaceAnonymity("char:viewer", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                observed.player().getUuidAsString(),
                false,
                observed.profileName(),
                "Awaken",
                "",
                List.of()
            )
        ));

        ObservedTarget target = observeAndRender(observed.player());

        assertPlayerSnapshot(context, target.state(), observed.player());
        assertEquals(
            context,
            observed.decoratedName(),
            target.state().displayName(),
            "exposed player snapshot must preserve the real scoreboard-decorated name"
        );
        context.assertTrue(
            containsText(target.commands(), observed.decoratedName()),
            "exposed player HUD must render the scoreboard-decorated name: " + observed.decoratedName()
        );
        context.assertFalse(
            containsText(target.commands(), TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME),
            "exposed player HUD must not render the anonymous placeholder"
        );
        context.complete();
    }

    @GameTest(templateName = FabricGameTest.EMPTY_STRUCTURE)
    public void unknownPlayerFailsClosedWithoutLeakingNames(TestContext context) {
        resetStores();
        ObservedPlayer observed = createObservedPlayer(context, "unknown-target", "[陌生队] ");

        ObservedTarget target = observeAndRender(observed.player());

        assertPlayerSnapshot(context, target.state(), observed.player());
        assertEquals(
            context,
            TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME,
            target.state().displayName(),
            "unknown player snapshot must fail closed to the anonymous placeholder"
        );
        context.assertTrue(
            containsText(target.commands(), TargetInfoState.ANONYMOUS_PLAYER_DISPLAY_NAME),
            "unknown player HUD must render the anonymous placeholder"
        );
        context.assertFalse(
            containsText(target.commands(), observed.profileName()),
            "unknown player HUD must not leak the real profile name: " + observed.profileName()
        );
        context.assertFalse(
            containsText(target.commands(), observed.decoratedName()),
            "unknown player HUD must not leak the scoreboard-decorated name: " + observed.decoratedName()
        );
        context.complete();
    }

    private static ObservedPlayer createObservedPlayer(
        TestContext context,
        String teamName,
        String teamPrefix
    ) {
        ServerPlayerEntity player = context.createMockCreativeServerPlayerInWorld();
        Team team = context.getWorld().getScoreboard().addTeam(teamName);
        team.setPrefix(Text.literal(teamPrefix));
        boolean joined = context.getWorld().getScoreboard().addPlayerToTeam(player.getEntityName(), team);
        context.assertTrue(
            joined,
            "real GameTest player must join scoreboard team " + teamName + " by entity/profile name"
        );

        String profileName = player.getGameProfile().getName();
        String decoratedName = teamPrefix + profileName;
        assertEquals(
            context,
            decoratedName,
            player.getDisplayName().getString(),
            "real ServerPlayerEntity must expose its scoreboard-decorated display name before TargetInfo masking"
        );
        return new ObservedPlayer(player, profileName, decoratedName);
    }

    private static ObservedTarget observeAndRender(ServerPlayerEntity player) {
        TargetInfoStateStore.observeEntity(player, OBSERVED_AT_MILLIS);
        TargetInfoState state = TargetInfoStateStore.snapshot();
        List<HudRenderCommand> commands = TargetInfoHudPlanner.buildCommands(
            state,
            RENDER_AT_MILLIS,
            FIXED_WIDTH,
            320,
            180
        );
        return new ObservedTarget(state, commands);
    }

    private static void assertPlayerSnapshot(
        TestContext context,
        TargetInfoState state,
        ServerPlayerEntity player
    ) {
        assertEquals(
            context,
            TargetInfoState.Kind.PLAYER,
            state.kind(),
            "observeEntity(PlayerEntity) must produce a PLAYER snapshot"
        );
        assertEquals(
            context,
            "entity:" + player.getId(),
            state.targetId(),
            "observeEntity(PlayerEntity) must preserve the real entity id"
        );
        context.assertFalse(
            state.isEmpty(),
            "observeEntity(PlayerEntity) must replace the TargetInfoStateStore snapshot"
        );
    }

    private static boolean containsText(List<HudRenderCommand> commands, String expected) {
        return commands.stream().anyMatch(command -> command.text().contains(expected));
    }

    private static void assertEquals(
        TestContext context,
        Object expected,
        Object actual,
        String message
    ) {
        context.assertTrue(
            java.util.Objects.equals(expected, actual),
            message + "; expected=" + expected + ", actual=" + actual
        );
    }

    private static void resetStores() {
        PlayerStateStore.resetForTests();
        NpcMoodStore.clearAll();
        NpcMetadataStore.clearAll();
        SocialStateStore.resetForTests();
        TargetInfoStateStore.resetForTests();
    }

    private record ObservedPlayer(
        ServerPlayerEntity player,
        String profileName,
        String decoratedName
    ) {
    }

    private record ObservedTarget(
        TargetInfoState state,
        List<HudRenderCommand> commands
    ) {
    }
}
