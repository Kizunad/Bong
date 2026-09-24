package com.bong.client.network;

import com.bong.client.combat.QuickUseSlotStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QuickSlotConfigHandlerTest {
    @AfterEach
    void tearDown() {
        QuickUseSlotStore.resetForTests();
    }

    @Test
    void authoritativeBindAckCarriesRequestCausalityIntoStoreUpdate() {
        AtomicReference<QuickUseSlotStore.Update> observed = new AtomicReference<>();
        QuickUseSlotStore.subscribeAndGet(observed::set);

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route("""
            {"v":1,"type":"quickslot_config",
             "eligible_item_ids":["guyuan_pill"],
             "slots":[{"instance_id":42,"stack_count":2,"item_id":"guyuan_pill","display_name":"土块","cast_duration_ms":1500,
                       "cooldown_ms":500,"icon_texture":""},null],
             "cooldown_until_ms":[0,0],
             "ack_request_id":"bind-42","bind_accepted":true}
            """, 0);

        assertTrue(result.isHandled(), result.logMessage());
        QuickUseSlotStore.Update update = observed.get();
        assertEquals(QuickUseSlotStore.Source.SERVER, update.source());
        assertEquals("bind-42", update.ackRequestId());
        assertEquals(Boolean.TRUE, update.bindAccepted());
        assertEquals("guyuan_pill", update.config().slot(0).itemId());
        assertEquals(42L, update.config().slot(0).instanceId());
        assertEquals(2, update.config().slot(0).stackCount());
        assertTrue(update.config().allowsItem("guyuan_pill"));
    }
}
