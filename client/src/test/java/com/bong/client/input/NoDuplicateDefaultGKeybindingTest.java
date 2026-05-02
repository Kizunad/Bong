package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class NoDuplicateDefaultGKeybindingTest {
    @Test
    void onlyUnifiedInteractionKeybindingUsesDefaultG() throws IOException {
        Path root = Path.of("src/main/java/com/bong/client");
        long occurrences;
        try (var files = Files.walk(root)) {
            occurrences = files
                .filter(path -> path.toString().endsWith(".java"))
                .map(NoDuplicateDefaultGKeybindingTest::read)
                .filter(text -> text.contains("GLFW.GLFW_KEY_G"))
                .count();
        }

        assertEquals(1, occurrences, "Only InteractionKeybindings may default an environment action to G");
    }

    private static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new IllegalStateException(exception);
        }
    }
}
