package com.bong.client.input;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class InteractionPriorityContractTest {
    @Test
    void priorityTableIsPinned() {
        assertEquals(100, ReservedInteractionIntents.SEARCH_CONTAINER_PRIORITY);
        assertEquals(90, ReservedInteractionIntents.TRADE_PLAYER_PRIORITY);
        assertEquals(90, ReservedInteractionIntents.TALK_NPC_PRIORITY);
        assertEquals(80, ReservedInteractionIntents.ACTIVATE_SHRINE_PRIORITY);
        assertEquals(70, ReservedInteractionIntents.PICKUP_DROPPED_ITEM_PRIORITY);
        assertEquals(60, ReservedInteractionIntents.HARVEST_RESOURCE_PRIORITY);
    }
}
