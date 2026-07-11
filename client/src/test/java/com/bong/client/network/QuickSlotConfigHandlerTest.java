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
             "slots":[{"item_id":"earth_crumb","display_name":"土块","cast_duration_ms":1500,
                       "cooldown_ms":500,"icon_texture":""},null,null,null,null,null,null,null,null],
             "cooldown_until_ms":[0,0,0,0,0,0,0,0,0],
             "ack_request_id":"bind-42","bind_accepted":true}
            """, 0);

        assertTrue(result.isHandled(), result.logMessage());
        QuickUseSlotStore.Update update = observed.get();
        assertEquals(QuickUseSlotStore.Source.SERVER, update.source());
        assertEquals("bind-42", update.ackRequestId());
        assertEquals(Boolean.TRUE, update.bindAccepted());
        assertEquals("earth_crumb", update.config().slot(0).itemId());
    }
}
