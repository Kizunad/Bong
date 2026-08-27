package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class KeybindMigrationPersistenceTest {
    @Test
    void propertiesServicePersistsMarkersAcrossReloadAndPreservesOtherMarkers() throws Exception {
        Path marker = Files.createTempDirectory("bong-keybind-marker-")
            .resolve("nested/migrations.properties");
        PropertiesKeybindMigrationPersistence first =
            new PropertiesKeybindMigrationPersistence(marker);

        assertFalse(first.hasCompleted("forge-open-screen-u-v1"),
            "a missing marker file must report the migration as incomplete");
        first.markCompleted("forge-open-screen-u-v1");
        first.markCompleted("another-migration-v1");

        PropertiesKeybindMigrationPersistence reloaded =
            new PropertiesKeybindMigrationPersistence(marker);
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
        KeybindMigrationPersistence persistence =
            new PropertiesKeybindMigrationPersistence(marker);

        assertThrows(NullPointerException.class, () -> persistence.hasCompleted(null));
        assertThrows(IllegalArgumentException.class, () -> persistence.hasCompleted(" "));
        assertThrows(NullPointerException.class, () -> persistence.markCompleted(null));
        assertThrows(IllegalArgumentException.class, () -> persistence.markCompleted("\t"));
    }

    @Test
    void malformedPropertiesFailClosedAsLifecycleSafePersistenceErrors() throws Exception {
        Path marker = Files.createTempDirectory("bong-keybind-marker-malformed-")
            .resolve("migrations.properties");
        Files.writeString(marker, "completed.broken=\\u12\n");
        KeybindMigrationPersistence persistence =
            new PropertiesKeybindMigrationPersistence(marker);

        IllegalStateException readFailure = assertThrows(IllegalStateException.class,
            () -> persistence.hasCompleted("forge-open-screen-u-v1"));
        assertTrue(readFailure.getMessage().contains("cannot read keybinding migration marker"),
            "malformed marker reads must use the lifecycle-safe persistence error contract");
        assertTrue(readFailure.getCause() instanceof IllegalArgumentException,
            "the malformed Unicode escape must remain available as the root cause");

        IllegalStateException updateFailure = assertThrows(IllegalStateException.class,
            () -> persistence.markCompleted("forge-open-screen-u-v1"));
        assertTrue(updateFailure.getMessage().contains("cannot update keybinding migration marker"),
            "updating a malformed marker must fail without replacing its existing contents");
        assertTrue(updateFailure.getCause() instanceof IllegalArgumentException,
            "the update path must preserve the malformed marker parse failure as its cause");
        assertTrue(Files.readString(marker).equals("completed.broken=\\u12\n"),
            "a rejected malformed marker must not be partially overwritten");
    }
}
