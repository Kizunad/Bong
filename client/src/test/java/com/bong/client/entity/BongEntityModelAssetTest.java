package com.bong.client.entity;

import com.bong.client.alchemy.AlchemyResultEffect;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongEntityModelAssetTest {
    private static final Path CLIENT_ROOT = resolveClientRoot();
    private static final Path RESOURCES = CLIENT_ROOT.resolve(Path.of("src", "main", "resources"));
    private static final Path MODEL_SOURCES = CLIENT_ROOT.getParent().resolve(Path.of("modelScript", "models"));

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
            Path file = MODEL_SOURCES.resolve(kind.blockbenchFileName());
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
            assertTrue(
                countOccurrences(body, "\"origin\"") >= 2,
                "Geo asset must have multiple modeled parts, not a single cube placeholder: " + kind.entityId()
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
            String compact = body.replaceAll("\\s+", "");
            assertTrue(compact.contains("\"loop\":true"), "Animation must loop for " + kind.entityId());
            assertTrue(
                !animatedBoneNames(body).isEmpty(),
                "Animation must target model bones for " + kind.entityId()
            );
            assertTrue(
                geoBoneNames(kind).containsAll(animatedBoneNames(body)),
                "Animation must only target bones present in geo asset for " + kind.entityId()
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

    @Test
    void alchemyResultAnimationsExistAndFinishBeforeTheirControllerReleasesThem() throws IOException {
        Path asset = RESOURCES.resolve("assets/bong/animations/alchemy_furnace.animation.json");
        var animations = JsonParser.parseString(Files.readString(asset)).getAsJsonObject().getAsJsonObject("animations");
        for (var effect : AlchemyResultEffect.values()) {
            assertTrue(animations.has(effect.animation()), "缺少结算动画资源：" + effect.animation());
            var clip = animations.getAsJsonObject(effect.animation());
            assertTrue(clip.getAsJsonObject("bones").size() > 0, "结算动画必须实际驱动骨骼：" + effect);
            assertTrue(clip.get("animation_length").getAsDouble() * 20 <= effect.ticks() + .001,
                "动画不能在尚未播完时被控制器切回待机：" + effect);
        }
    }

    private static int countOccurrences(String body, String needle) {
        int count = 0;
        int offset = 0;
        while (true) {
            int index = body.indexOf(needle, offset);
            if (index < 0) {
                return count;
            }
            count++;
            offset = index + needle.length();
        }
    }

    private static Set<String> geoBoneNames(BongEntityModelKind kind) throws IOException {
        Path geo = RESOURCES.resolve(Path.of("assets", "bong", "geo", kind.entityId() + ".geo.json"));
        Matcher matcher = Pattern.compile("\"name\"\\s*:\\s*\"([^\"]+)\"")
            .matcher(Files.readString(geo, StandardCharsets.UTF_8));
        Set<String> names = new HashSet<>();
        while (matcher.find()) {
            names.add(matcher.group(1));
        }
        return names;
    }

    private static Set<String> animatedBoneNames(String body) {
        Matcher matcher = Pattern.compile("\"([^\"]+)\"\\s*:\\s*\\{\\s*\"(?:rotation|position|scale)\"")
            .matcher(body);
        Set<String> names = new HashSet<>();
        while (matcher.find()) {
            names.add(matcher.group(1));
        }
        return names;
    }
}
