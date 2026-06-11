package com.bong.client.block;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongBlocksTest {
    private static final Path CLIENT_ROOT = resolveClientRoot();
    private static final Path REPO_ROOT = CLIENT_ROOT.getParent();
    private static final Path RESOURCES = CLIENT_ROOT.resolve(Path.of("src", "main", "resources"));

    @Test
    void idsAreGeneratedFromBongBlockManifest() throws IOException {
        String manifest = Files.readString(REPO_ROOT.resolve("bong_blocks.json"), StandardCharsets.UTF_8);

        assertManifestContainsName(manifest, "zhenfa_node");
        assertManifestContainsName(manifest, "zhenfa_line");
        assertManifestContainsName(manifest, "zhenfa_eye");
        assertManifestContainsName(manifest, "simple_bed");
        assertManifestContainsName(manifest, "meditation_mat");
        assertManifestContainsName(manifest, "moisture_base");
        assertManifestContainsName(manifest, "spirit_stone_rack");
        assertManifestContainsName(manifest, "axis");
        assertManifestContainsName(manifest, "charged");

        // These values intentionally pin protocol IDs; failures mean manifest order or base IDs changed.
        assertEquals(1003, BongBlockIds.BONG_BLOCK_ID_START);
        assertEquals(24135, BongBlockIds.BONG_BLOCK_STATE_START);
        assertEquals(1003, BongBlockIds.ZHENFA_NODE_BLOCK_ID);
        assertEquals(1004, BongBlockIds.ZHENFA_LINE_BLOCK_ID);
        assertEquals(1005, BongBlockIds.ZHENFA_EYE_BLOCK_ID);
        assertEquals(1006, BongBlockIds.SIMPLE_BED_BLOCK_ID);
        assertEquals(1007, BongBlockIds.MEDITATION_MAT_BLOCK_ID);
        assertEquals(1008, BongBlockIds.MOISTURE_BASE_BLOCK_ID);
        assertEquals(1009, BongBlockIds.SPIRIT_STONE_RACK_BLOCK_ID);
        assertEquals(24135, BongBlockIds.ZHENFA_NODE_STATE_ID);
        assertEquals(24136, BongBlockIds.ZHENFA_LINE_STATE_ID);
        assertEquals(24139, BongBlockIds.ZHENFA_EYE_STATE_ID);
        assertEquals(24141, BongBlockIds.SIMPLE_BED_STATE_ID);
        assertEquals(24142, BongBlockIds.MEDITATION_MAT_STATE_ID);
        assertEquals(24143, BongBlockIds.MOISTURE_BASE_STATE_ID);
        assertEquals(24144, BongBlockIds.SPIRIT_STONE_RACK_STATE_ID);
        assertEquals(1, BongBlockIds.ZHENFA_NODE_STATE_COUNT);
        assertEquals(3, BongBlockIds.ZHENFA_LINE_STATE_COUNT);
        assertEquals(2, BongBlockIds.ZHENFA_EYE_STATE_COUNT);
        assertEquals(1, BongBlockIds.SIMPLE_BED_STATE_COUNT);
        assertEquals(1, BongBlockIds.MEDITATION_MAT_STATE_COUNT);
        assertEquals(1, BongBlockIds.MOISTURE_BASE_STATE_COUNT);
        assertEquals(1, BongBlockIds.SPIRIT_STONE_RACK_STATE_COUNT);
    }

    @Test
    void blockAssetsExistForAllManifestEntries() {
        for (String id : List.of(
            "zhenfa_node",
            "zhenfa_line",
            "zhenfa_eye",
            "simple_bed",
            "meditation_mat",
            "moisture_base",
            "spirit_stone_rack"
        )) {
            assertTrue(
                Files.exists(RESOURCES.resolve(Path.of("assets", "bong", "blockstates", id + ".json"))),
                "Missing blockstate asset for " + id
            );
            assertTrue(
                Files.exists(RESOURCES.resolve(Path.of("assets", "bong", "models", "block", id + ".json"))),
                "Missing block model asset for " + id
            );
            assertTrue(
                Files.exists(RESOURCES.resolve(Path.of("assets", "bong", "textures", "block", id + ".png"))),
                "Missing block texture for " + id
            );
        }
    }

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

    private static void assertManifestContainsName(String manifest, String name) {
        assertTrue(
            manifest.contains("\"name\": \"" + name + "\""),
            "expected manifest to contain \"" + name + "\" because Bong block IDs are generated from explicit manifest names, actual manifest: " + manifest
        );
    }
}
