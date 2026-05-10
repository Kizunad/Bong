package com.bong.client.entity;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongEntityModelAssetTest {
    private static final Path CLIENT_ROOT = resolveClientRoot();
    private static final Path RESOURCES = CLIENT_ROOT.resolve(Path.of("src", "main", "resources"));
    private static final Path LOCAL_MODELS = CLIENT_ROOT.getParent().resolve("local_models");

    private static Path resolveClientRoot() {
        Path cwd = Path.of("").toAbsolutePath().normalize();
        if (Files.isDirectory(cwd.resolve(Path.of("src", "main", "resources")))) {
            return cwd;
        }
        Path nestedClient = cwd.resolve("client");
        if (Files.isDirectory(nestedClient.resolve(Path.of("src", "main", "resources")))) {
            return nestedClient;
        }
        throw new IllegalStateException("Cannot locate client module root from " + cwd);
    }

    @Test
    void blockbenchSourcesExistForEveryGameEntity() {
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            Path file = LOCAL_MODELS.resolve(kind.blockbenchFileName());
            assertTrue(Files.exists(file), "Missing BlockBench source: " + file.toAbsolutePath());
        }
    }

    @Test
    void geckoModelAssetsExistForEveryGameEntity() throws IOException {
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            Path model = RESOURCES.resolve(Path.of("assets", "bong", "geo", kind.entityId() + ".geo.json"));
            assertTrue(Files.exists(model), "Missing GeckoLib geo asset: " + model.toAbsolutePath());
            String body = Files.readString(model, StandardCharsets.UTF_8);
            assertTrue(
                body.contains("geometry.bong." + kind.entityId()),
                "Geo asset must expose geometry.bong." + kind.entityId()
            );
        }
    }

    @Test
    void animationAssetsExposeIdleAnimationForEveryGameEntity() throws IOException {
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            Path animation = RESOURCES.resolve(Path.of("assets", "bong", "animations", kind.entityId() + ".animation.json"));
            assertTrue(Files.exists(animation), "Missing GeckoLib animation asset: " + animation.toAbsolutePath());
            String body = Files.readString(animation, StandardCharsets.UTF_8);
            assertTrue(
                body.contains(kind.idleAnimationName()),
                "Animation asset must expose " + kind.idleAnimationName()
            );
        }
    }

    @Test
    void stateTexturesExistForEveryGameEntity() {
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            for (String state : kind.textureStates()) {
                Path texture = RESOURCES.resolve(Path.of(
                    "assets",
                    "bong",
                    "textures",
                    "entity",
                    kind.entityId() + "_" + state + ".png"
                ));
                assertTrue(Files.exists(texture), "Missing state texture: " + texture.toAbsolutePath());
            }
        }
    }
}
