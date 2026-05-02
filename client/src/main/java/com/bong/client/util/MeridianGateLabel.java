package com.bong.client.util;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;

public final class MeridianGateLabel {
    public static final int SPIRIT_EXTRAORDINARY_REQUIRED = 4;

    private MeridianGateLabel() {}

    public static String spiritExtraordinaryProgress(MeridianBody body) {
        return "奇经 " + openedExtraordinaryCount(body) + "/" + SPIRIT_EXTRAORDINARY_REQUIRED;
    }

    public static int openedExtraordinaryCount(MeridianBody body) {
        if (body == null) {
            return 0;
        }

        int opened = 0;
        for (var entry : body.allChannels().entrySet()) {
            MeridianChannel channel = entry.getKey();
            ChannelState state = entry.getValue();
            if (channel.family() == MeridianChannel.Family.EXTRAORDINARY
                && state != null
                && !state.blocked()) {
                opened++;
            }
        }
        return opened;
    }

    public static boolean countsTowardSpiritGate(MeridianChannel channel) {
        return channel != null && channel.family() == MeridianChannel.Family.EXTRAORDINARY;
    }
}
