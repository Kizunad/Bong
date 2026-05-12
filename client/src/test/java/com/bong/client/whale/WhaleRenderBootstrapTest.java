package com.bong.client.whale;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class WhaleRenderBootstrapTest {
    @Test
    void whaleEntityPinsCurrentClientRawId() {
        assertEquals(133, WhaleEntities.EXPECTED_RAW_ID);
    }
}
