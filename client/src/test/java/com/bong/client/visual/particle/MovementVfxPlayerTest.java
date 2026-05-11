package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertThrows;

public class MovementVfxPlayerTest {
    @Test
    void constructorRejectsNullKind() {
        assertThrows(NullPointerException.class, () -> new MovementVfxPlayer(null));
    }
}
