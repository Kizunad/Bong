package com.bong.client.input;

import net.fabricmc.loader.api.FabricLoader;

import java.io.IOException;
import java.io.Reader;
import java.io.Writer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import java.util.Properties;

/** Persistent completion markers used internally by {@link KeybindMigrationService}. */
interface KeybindMigrationPersistence {
    boolean hasCompleted(String migrationId);

    void markCompleted(String migrationId);

    /** Creates production persistence; its config-file location is an implementation detail. */
    static KeybindMigrationPersistence clientConfig() {
        Path markerFile = FabricLoader.getInstance().getConfigDir()
            .resolve("bong-client-keybind-migrations.properties");
        return new PropertiesKeybindMigrationPersistence(markerFile);
    }
}

final class PropertiesKeybindMigrationPersistence implements KeybindMigrationPersistence {
    private final Path markerFile;

    PropertiesKeybindMigrationPersistence(Path markerFile) {
        this.markerFile = Objects.requireNonNull(markerFile, "marker file must not be null");
    }

    @Override
    public synchronized boolean hasCompleted(String migrationId) {
        String property = markerProperty(migrationId);
        if (!Files.isRegularFile(markerFile)) {
            return false;
        }
        Properties markers = new Properties();
        try (Reader reader = Files.newBufferedReader(markerFile)) {
            markers.load(reader);
            return "true".equals(markers.getProperty(property));
        } catch (IOException | IllegalArgumentException exception) {
            throw new IllegalStateException(
                "cannot read keybinding migration marker: " + markerFile, exception
            );
        }
    }

    @Override
    public synchronized void markCompleted(String migrationId) {
        String property = markerProperty(migrationId);
        Properties markers = new Properties();
        if (Files.isRegularFile(markerFile)) {
            try (Reader reader = Files.newBufferedReader(markerFile)) {
                markers.load(reader);
            } catch (IOException | IllegalArgumentException exception) {
                throw new IllegalStateException(
                    "cannot update keybinding migration marker: " + markerFile, exception
                );
            }
        }
        markers.setProperty(property, "true");
        Path parent = markerFile.getParent();
        try {
            if (parent != null) {
                Files.createDirectories(parent);
            }
            try (Writer writer = Files.newBufferedWriter(markerFile)) {
                markers.store(writer, "Bong client keybinding migrations");
            }
        } catch (IOException exception) {
            throw new IllegalStateException(
                "cannot write keybinding migration marker: " + markerFile, exception
            );
        }
    }

    private static String markerProperty(String migrationId) {
        Objects.requireNonNull(migrationId, "migration id must not be null");
        if (migrationId.isBlank()) {
            throw new IllegalArgumentException("migration id must not be blank");
        }
        return "completed." + migrationId;
    }
}
