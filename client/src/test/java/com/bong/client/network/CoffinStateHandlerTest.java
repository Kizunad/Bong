package com.bong.client.network;

import com.bong.client.hud.CoffinStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CoffinStateHandlerTest {
    @AfterEach
    void resetStore() {
        CoffinStateStore.resetForTests();
    }

    @Test
    void acceptsCoffinStatePayload() {
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":true,"lifespan_rate_multiplier":0.9}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled());
        assertTrue(CoffinStateStore.snapshot().inCoffin());
        assertEquals(0.9, CoffinStateStore.snapshot().lifespanRateMultiplier(), 1e-9);
    }

    @Test
    void rejectsInvalidMultiplier() {
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":true,"lifespan_rate_multiplier":0.0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp());
        assertFalse(CoffinStateStore.snapshot().inCoffin());
    }

    // ─── plan-coffin-tiers-v1 P3：coffin_grade 解析 ───────────────────────────

    @ParameterizedTest(name = "grade={0} → store.grade()={1} multiplier={2}")
    @CsvSource({
        "mundane, mundane, 0.9",
        "jade,    jade,    0.7",
        "stone,   stone,   0.5",
        "bronze,  bronze,  0.3"
    })
    void parsesAllFourGrades(String wireGrade, String expectedGrade, double expectedMultiplier) {
        String json = String.format(
            "{\"type\":\"coffin_state\",\"v\":1,\"in_coffin\":true,"
                + "\"lifespan_rate_multiplier\":%s,\"coffin_grade\":\"%s\"}",
            expectedMultiplier, wireGrade
        );

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(),
            "coffin_state with grade=" + wireGrade + " should be handled, got noOp");
        assertEquals(expectedGrade, CoffinStateStore.snapshot().grade(),
            "grade=" + wireGrade + " should store as '" + expectedGrade + "'");
        assertEquals(expectedMultiplier, CoffinStateStore.snapshot().lifespanRateMultiplier(), 1e-9,
            "multiplier should be " + expectedMultiplier + " for grade=" + wireGrade);
    }

    @Test
    void missingCoffinGradeStoresNull() {
        // coffin_grade 字段缺失（不在棺内，server 不发该字段）→ grade=null。
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":false,"lifespan_rate_multiplier":1.0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "valid coffin_state without grade should be handled");
        assertNull(CoffinStateStore.snapshot().grade(),
            "missing coffin_grade should store null, not a default string");
    }

    @Test
    void unknownCoffinGradeStoresNull() {
        // 未知档级（前向兼容）→ null，不崩溃，不拒绝整个 payload。
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":true,"lifespan_rate_multiplier":0.9,"coffin_grade":"diamond"}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "unknown grade should not reject the payload");
        assertNull(CoffinStateStore.snapshot().grade(),
            "unknown grade 'diamond' should store null (forward-compat defense)");
    }

    @Test
    void coffinGradeNullFieldStoresNull() {
        // coffin_grade: null（JSON null）→ grade=null，与缺失字段一致。
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":false,"lifespan_rate_multiplier":1.0,"coffin_grade":null}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "null grade field should be handled");
        assertNull(CoffinStateStore.snapshot().grade(),
            "JSON null coffin_grade should store null");
    }
}
