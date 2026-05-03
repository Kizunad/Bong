package com.bong.client.network;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.cultivation.QiColorObservedStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

class QiColorObservedHandlerTest {
    @BeforeEach
    void setUp() {
        QiColorObservedStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        QiColorObservedStore.resetForTests();
    }

    @Test
    void routesObservedQiColorIntoStore() {
        String json = """
            {
              "v": 1,
              "type": "qi_color_observed",
              "observer": "offline:Observer",
              "observed": "offline:Observed",
              "main": "Intricate",
              "secondary": "Heavy",
              "is_chaotic": true,
              "is_hunyuan": false,
              "realm_diff": 2
            }
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), result.dispatch().logMessage());
        var snapshot = QiColorObservedStore.snapshot();
        assertNotNull(snapshot);
        assertEquals("offline:Observer", snapshot.observer());
        assertEquals("offline:Observed", snapshot.observed());
        assertEquals(ColorKind.Intricate, snapshot.main());
        assertEquals(ColorKind.Heavy, snapshot.secondary());
        assertTrue(snapshot.chaotic());
        assertFalse(snapshot.hunyuan());
        assertEquals(2.0, snapshot.realmDiff(), 1e-9);
        assertEquals("对方真元 巧/厚 · 杂色", snapshot.displayText());
    }

    @Test
    void rejectsUnknownMainColorWithoutTouchingStore() {
        String json = """
            {
              "v": 1,
              "type": "qi_color_observed",
              "observer": "offline:Observer",
              "observed": "offline:Observed",
              "main": "Unknown",
              "is_chaotic": false,
              "is_hunyuan": false,
              "realm_diff": 1
            }
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), result.dispatch().logMessage());
        assertNull(QiColorObservedStore.snapshot());
    }
}
