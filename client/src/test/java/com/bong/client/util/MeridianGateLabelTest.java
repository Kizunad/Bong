package com.bong.client.util;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class MeridianGateLabelTest {
    @Test
    void openedExtraordinaryCountIgnoresRegularAndBlockedChannels() {
        MeridianBody body = MeridianBody.builder()
            .channel(ChannelState.healthy(MeridianChannel.LU, 10.0))
            .channel(ChannelState.healthy(MeridianChannel.REN, 10.0))
            .channel(ChannelState.healthy(MeridianChannel.DU, 10.0))
            .channel(new ChannelState(
                MeridianChannel.CHONG,
                10.0,
                0.0,
                ChannelState.DamageLevel.INTACT,
                0.0,
                0.0,
                true
            ))
            .build();

        assertEquals(2, MeridianGateLabel.openedExtraordinaryCount(body));
        assertEquals("奇经 2/4", MeridianGateLabel.spiritExtraordinaryProgress(body));
    }

    @Test
    void countsTowardSpiritGateOnlyAcceptsExtraordinaryChannels() {
        assertTrue(MeridianGateLabel.countsTowardSpiritGate(MeridianChannel.REN));
        assertFalse(MeridianGateLabel.countsTowardSpiritGate(MeridianChannel.LU));
    }
}
