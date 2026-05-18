package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3/P4 -- Asset existence tests for dandao mutation
 * models + textures and baolongwang BOSS model.
 */
class DandaoAssetTest {
    private static final Path CLIENT_ROOT = resolveClientRoot();
    private static final Path RESOURCES = CLIENT_ROOT.resolve(Path.of("src", "main", "resources"));

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

    // -- Mutation attachment models (10 geo.json files from PR-5) --

    @Test
    void allMutationGeoAssetsExist() {
        String[] expectedGeo = {
            "dandao_golden_iris",
            "dandao_hardened_nails",
            "dandao_bone_ridge",
            "dandao_forearm_scales",
            "dandao_spine_spurs",
            "dandao_horns",
            "dandao_tail",
            "dandao_carapace",
            "dandao_extra_arms",
            "dandao_beast_face"
        };

        for (String name : expectedGeo) {
            Path geoFile = RESOURCES.resolve(Path.of("assets", "bong", "geo", name + ".geo.json"));
            assertTrue(Files.exists(geoFile),
                "Missing dandao geo asset: " + geoFile.toAbsolutePath());
        }
    }

    @Test
    void allMutationTexturesExist() {
        String[] expectedTextures = {
            "dandao_iris",
            "dandao_nails",
            "dandao_ridge",
            "dandao_scales",
            "dandao_spurs",
            "dandao_horns",
            "dandao_tail",
            "dandao_carapace",
            "dandao_arms",
            "dandao_beast"
        };

        for (String name : expectedTextures) {
            Path texFile = RESOURCES.resolve(Path.of("assets", "bong", "textures", "entity", "mutation", name + ".png"));
            assertTrue(Files.exists(texFile),
                "Missing dandao mutation texture: " + texFile.toAbsolutePath());
        }
    }

    @Test
    void mutationGeoAssetsHaveValidJson() throws IOException {
        for (MutationKind kind : MutationKind.values()) {
            String geoPath = kind.geoId().getPath();
            Path geoFile = RESOURCES.resolve(Path.of("assets", "bong", geoPath));
            if (!Files.exists(geoFile)) {
                continue; // covered by allMutationGeoAssetsExist
            }
            String body = Files.readString(geoFile, StandardCharsets.UTF_8);
            assertTrue(body.contains("\"format_version\""),
                "Geo asset " + geoPath + " should have format_version");
            assertTrue(body.contains("minecraft:geometry"),
                "Geo asset " + geoPath + " should have minecraft:geometry");
        }
    }

    // -- Baolongwang BOSS assets --

    @Test
    void baolongwangGeoExists() {
        Path geo = RESOURCES.resolve(Path.of("assets", "bong", "geo", "baolongwang.geo.json"));
        assertTrue(Files.exists(geo),
            "Missing baolongwang geo asset: " + geo.toAbsolutePath());
    }

    @Test
    void baolongwangTextureExists() {
        Path tex = RESOURCES.resolve(Path.of("assets", "bong", "textures", "entity", "baolongwang.png"));
        assertTrue(Files.exists(tex),
            "Missing baolongwang texture: " + tex.toAbsolutePath());
    }

    @Test
    void baolongwangAnimationExists() {
        Path anim = RESOURCES.resolve(Path.of("assets", "bong", "animations", "baolongwang.animation.json"));
        assertTrue(Files.exists(anim),
            "Missing baolongwang animation: " + anim.toAbsolutePath());
    }

    @Test
    void baolongwangGeoHasExpectedBones() throws IOException {
        Path geo = RESOURCES.resolve(Path.of("assets", "bong", "geo", "baolongwang.geo.json"));
        String body = Files.readString(geo, StandardCharsets.UTF_8);

        // Verify key bones from §8.1 #6
        String[] expectedBones = {"bdk_body", "bdk_rl", "bdk_ll", "bdk_ra", "bdk_la", "bdk_rw", "bdk_lw"};
        for (String bone : expectedBones) {
            assertTrue(body.contains("\"" + bone + "\""),
                "Baolongwang geo should contain bone '" + bone + "'");
        }
    }

    @Test
    void baolongwangAnimationHasExpectedAnimations() throws IOException {
        Path anim = RESOURCES.resolve(Path.of("assets", "bong", "animations", "baolongwang.animation.json"));
        String body = Files.readString(anim, StandardCharsets.UTF_8);

        // 5 animations per §5.1
        assertTrue(body.contains("\"idle\""), "Should have idle animation");
        assertTrue(body.contains("\"walk\""), "Should have walk animation");
        assertTrue(body.contains("\"attack\""), "Should have attack animation");
        assertTrue(body.contains("\"skill1\""), "Should have skill1 animation");
        assertTrue(body.contains("\"skill2\""), "Should have skill2 animation");
    }

    @Test
    void baolongwangAnimationIsLoopingForIdleAndWalk() throws IOException {
        Path anim = RESOURCES.resolve(Path.of("assets", "bong", "animations", "baolongwang.animation.json"));
        String body = Files.readString(anim, StandardCharsets.UTF_8);

        // Verify idle and walk specifically have "loop": true.
        // A naive contains("\"loop\"") is a false positive — non-looping
        // animations also contain "loop": false.
        // Parse the animation entries to check per-animation loop status.
        var root = com.google.gson.JsonParser.parseString(body).getAsJsonObject();
        var animations = root.getAsJsonObject("animations");

        var idle = animations.getAsJsonObject("idle");
        assertNotNull(idle, "Should have 'idle' animation entry");
        assertTrue(idle.has("loop") && idle.get("loop").getAsBoolean(),
            "idle animation must have \"loop\": true (4.76s breathing loop)");

        var walk = animations.getAsJsonObject("walk");
        assertNotNull(walk, "Should have 'walk' animation entry");
        assertTrue(walk.has("loop") && walk.get("loop").getAsBoolean(),
            "walk animation must have \"loop\": true (1.12s walk cycle)");

        // attack, skill1, skill2 must NOT be looping
        for (String oneShot : new String[]{"attack", "skill1", "skill2"}) {
            var entry = animations.getAsJsonObject(oneShot);
            assertNotNull(entry, "Should have '" + oneShot + "' animation entry");
            assertFalse(entry.has("loop") && entry.get("loop").getAsBoolean(),
                oneShot + " animation must not be looping (one-shot action)");
        }
    }

    @Test
    void baolongwangTextureSizeLargeEnough() throws IOException {
        Path tex = RESOURCES.resolve(Path.of("assets", "bong", "textures", "entity", "baolongwang.png"));
        long size = Files.size(tex);
        assertTrue(size > 1000,
            "Baolongwang texture should be a real image (>1KB), got " + size + " bytes");
    }
}
