package com.bong.client.input;

import java.util.Objects;
import java.util.function.BooleanSupplier;

/** Coordinates versioned, one-time client keybinding migrations. */
public final class KeybindMigrationService {
    private final KeybindMigrationPersistence persistence;

    KeybindMigrationService(KeybindMigrationPersistence persistence) {
        this.persistence = Objects.requireNonNull(
            persistence, "migration persistence must not be null"
        );
    }

    /** Creates the production service backed by the client config directory. */
    public static KeybindMigrationService clientConfig() {
        return new KeybindMigrationService(KeybindMigrationPersistence.clientConfig());
    }

    /**
     * Runs a migration at most once for the supplied version id.
     *
     * <p>A completed no-op is still marked so a player may deliberately choose
     * the legacy key later.  Failed migration actions are not marked and may be
     * retried on the next startup.</p>
     */
    public synchronized boolean migrateOnce(String migrationId, BooleanSupplier migration) {
        requireNonBlank(migrationId);
        Objects.requireNonNull(migration, "migration must not be null");
        if (persistence.hasCompleted(migrationId)) {
            return false;
        }

        boolean migrated = migration.getAsBoolean();
        persistence.markCompleted(migrationId);
        return migrated;
    }

    private static void requireNonBlank(String migrationId) {
        Objects.requireNonNull(migrationId, "migration id must not be null");
        if (migrationId.isBlank()) {
            throw new IllegalArgumentException("migration id must not be blank");
        }
    }
}
