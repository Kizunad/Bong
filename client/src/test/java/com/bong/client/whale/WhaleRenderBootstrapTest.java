package com.bong.client.whale;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class WhaleRenderBootstrapTest {
    @Test
    void whaleEntityPinsCurrentClientRawId() {
        assertEquals(
            125,
            WhaleEntities.EXPECTED_RAW_ID,
            "expected 125 because whale raw id must stay immediately before fauna ids, actual: "
                + WhaleEntities.EXPECTED_RAW_ID
        );
    }
}
