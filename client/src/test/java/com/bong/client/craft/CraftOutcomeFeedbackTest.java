package com.bong.client.craft;

import org.junit.jupiter.api.Test;
import java.util.ArrayList;
import java.util.List;
import static org.junit.jupiter.api.Assertions.assertEquals;

class CraftOutcomeFeedbackTest {
    @Test
    void completionFlashesAndSoundsButFailureOnlyRefreshes() {
        List<String> events = new ArrayList<>();
        CraftOutcomeFeedback.apply(CraftStore.CraftOutcomeEvent.completed("recipe", "out", 1, 10),
            ticks -> events.add("flash"), () -> events.add("sound"), () -> events.add("refresh"));
        assertEquals(List.of("flash", "sound", "refresh"), events);
        events.clear();
        CraftOutcomeFeedback.apply(CraftStore.CraftOutcomeEvent.failed("recipe", "cancelled", 2, 0),
            ticks -> events.add("flash"), () -> events.add("sound"), () -> events.add("refresh"));
        assertEquals(List.of("refresh"), events, "失败不能发出成功的视听反馈");
    }
}
