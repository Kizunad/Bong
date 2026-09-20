package com.bong.client.cultivation;

import com.bong.client.inventory.model.MeridianChannel;

public record CultivationIntent(Action action, MeridianChannel channel) implements com.bong.client.ui.contract.UiIntent {
    public enum Action { TARGET, BREAKTHROUGH, DU_XU, FORGE_RATE, FORGE_CAPACITY }
}
