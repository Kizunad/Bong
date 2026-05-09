package com.bong.client.hud;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

public class MeridianOpenHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @BeforeEach
    void setUp() { MeridianStateStore.resetForTests(); }

    @AfterEach
    void tearDown() { MeridianStateStore.resetForTests(); }

    private static MeridianBody bodyWithTarget(MeridianChannel target, double progress) {
        MeridianBody.Builder builder = MeridianBody.builder();
        for (MeridianChannel ch : MeridianChannel.values()) {
            boolean isTarget = ch == target;
            builder.channel(new ChannelState(
                ch,
                isTarget ? 0.0 : 10.0,
                isTarget ? 0.0 : 10.0,
                ChannelState.DamageLevel.INTACT,
                0.0,
                isTarget ? progress : 0.0,
                isTarget
            ));
        }
        builder.targetMeridian(target);
        return builder.build();
    }

    @Test
    void showsProgressBarWhenTargetSet() {
        MeridianStateStore.replace(bodyWithTarget(MeridianChannel.LU, 0.42));
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isRect));
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("冲脉") && cmd.text().contains("肺经")));
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("42%")));
    }

    @Test
    void emptyWhenNoTarget() {
        MeridianBody.Builder builder = MeridianBody.builder();
        for (MeridianChannel ch : MeridianChannel.values()) {
            builder.channel(ChannelState.healthy(ch, 10.0));
        }
        MeridianStateStore.replace(builder.build());
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.isEmpty());
    }

    @Test
    void emptyWhenTargetAlreadyOpened() {
        MeridianBody.Builder builder = MeridianBody.builder();
        for (MeridianChannel ch : MeridianChannel.values()) {
            builder.channel(ChannelState.healthy(ch, 10.0));
        }
        builder.targetMeridian(MeridianChannel.LU);
        MeridianStateStore.replace(builder.build());
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.isEmpty());
    }

    @Test
    void emptyWhenNoSnapshot() {
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.isEmpty());
    }

    @Test
    void emptyWhenScreenSizeZero() {
        MeridianStateStore.replace(bodyWithTarget(MeridianChannel.HT, 0.5));
        assertTrue(MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 0, 0).isEmpty());
    }

    @Test
    void showsMeditationHint() {
        MeridianStateStore.replace(bodyWithTarget(MeridianChannel.KI, 0.1));
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("静坐吸灵中")));
    }

    @Test
    void extraordinaryMeridianShown() {
        MeridianStateStore.replace(bodyWithTarget(MeridianChannel.REN, 0.88));
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("任脉") && cmd.text().contains("88%")));
    }

    @Test
    void progressClampedAt100Percent() {
        MeridianStateStore.replace(bodyWithTarget(MeridianChannel.SP, 1.0));
        List<HudRenderCommand> commands = MeridianOpenHudPlanner.buildCommands(FIXED_WIDTH, 960, 540);
        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("100%")));
    }
}
