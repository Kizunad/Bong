package com.bong.client.alchemy;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyWindowContentTest {
    @Test
    void rejectedAlchemyIntentIsNotConsumedAsAnInventoryDrop() {
        assertTrue(AlchemyWindowContent.acceptedDrop(UiIntentResult.accepted("request-1")));
        assertFalse(AlchemyWindowContent.acceptedDrop(UiIntentResult.rejected("炉次已结束")));
        assertFalse(AlchemyWindowContent.acceptedDrop(UiIntentResult.error("transport")));
    }
}
