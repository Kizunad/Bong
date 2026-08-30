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
     * the legacy key later. Failed migration actions are not marked and may be
     * retried on the next startup. If the action changes state but its completion
     * marker cannot be persisted, the supplied rollback restores the pre-migration
     * state before the persistence failure is rethrown.</p>
     */
    public synchronized boolean migrateOnce(
        String migrationId,
        BooleanSupplier migration,
        Runnable rollback
    ) {
        requireNonBlank(migrationId);
        Objects.requireNonNull(migration, "migration must not be null");
        Objects.requireNonNull(rollback, "rollback must not be null");
        if (persistence.hasCompleted(migrationId)) {
            return false;
        }

        boolean migrated = migration.getAsBoolean();
        try {
            persistence.markCompleted(migrationId);
        } catch (IllegalStateException persistenceFailure) {
            if (migrated) {
                try {
                    rollback.run();
                } catch (RuntimeException rollbackFailure) {
                    persistenceFailure.addSuppressed(rollbackFailure);
                }
            }
            throw persistenceFailure;
        }
        return migrated;
    }

    private static void requireNonBlank(String migrationId) {
        Objects.requireNonNull(migrationId, "migration id must not be null");
        if (migrationId.isBlank()) {
            throw new IllegalArgumentException("migration id must not be blank");
        }
    }
}
