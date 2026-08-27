package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class KeybindMigrationStoreTest {
    @Test
    void propertiesStorePersistsMarkersAcrossReloadAndPreservesOtherMarkers() throws Exception {
        Path marker = Files.createTempDirectory("bong-keybind-marker-")
            .resolve("nested/migrations.properties");
        PropertiesKeybindMigrationStore first = new PropertiesKeybindMigrationStore(marker);

        assertFalse(first.hasCompleted("forge-open-screen-u-v1"),
            "a missing marker file must report the migration as incomplete");
        first.markCompleted("forge-open-screen-u-v1");
        first.markCompleted("another-migration-v1");

        PropertiesKeybindMigrationStore reloaded = new PropertiesKeybindMigrationStore(marker);
        assertTrue(reloaded.hasCompleted("forge-open-screen-u-v1"),
            "a completed marker must survive construction of a new persistence service");
        assertTrue(reloaded.hasCompleted("another-migration-v1"),
            "writing a later marker must preserve previously completed migrations");
        assertTrue(Files.readString(marker).contains("completed.forge-open-screen-u-v1=true"),
            "the file-backed service must persist the versioned completion property");
    }

    @Test
    void invalidMigrationIdsFailAtThePersistenceBoundary() throws Exception {
        Path marker = Files.createTempDirectory("bong-keybind-marker-invalid-")
            .resolve("migrations.properties");
        KeybindMigrationStore store = new PropertiesKeybindMigrationStore(marker);

        assertThrows(NullPointerException.class, () -> store.hasCompleted(null));
        assertThrows(IllegalArgumentException.class, () -> store.hasCompleted(" "));
        assertThrows(NullPointerException.class, () -> store.markCompleted(null));
        assertThrows(IllegalArgumentException.class, () -> store.markCompleted("\t"));
    }
}
