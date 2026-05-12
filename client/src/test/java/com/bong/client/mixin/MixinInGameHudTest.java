package com.bong.client.mixin;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

final class MixinInGameHudTest {
    @Test
    void registersHotbarStatusBarsAndExperienceCancels() throws IOException {
        String source = Files.readString(Path.of(
            "src/main/java/com/bong/client/mixin/MixinInGameHud.java"));

        assertTrue(source.contains("@Inject(method = \"renderHotbar\""));
        assertTrue(source.contains("@Inject(method = \"renderStatusBars\""));
        assertTrue(source.contains("@Inject(method = \"renderExperienceBar\""));
        assertTrue(source.contains("private void bong$replaceHotbar"));
        assertTrue(source.contains("private void bong$hideStatusBars"));
        assertTrue(source.contains("private void bong$hideExperienceBar"));
    }
}
