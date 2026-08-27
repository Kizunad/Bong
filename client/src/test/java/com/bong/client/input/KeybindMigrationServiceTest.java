package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class KeybindMigrationServiceTest {
    private static final String MIGRATION_ID = "forge-open-screen-u-v1";

    @Test
    void firstRunExecutesAndMarksCompletedMigrationExactlyOnce() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);
        AtomicInteger actions = new AtomicInteger();

        assertTrue(service.migrateOnce(MIGRATION_ID, () -> {
            actions.incrementAndGet();
            return true;
        }, () -> {}), "the first successful migration must report that it changed the binding");
        assertFalse(service.migrateOnce(MIGRATION_ID, () -> {
            actions.incrementAndGet();
            return true;
        }, () -> {}), "a completed migration id must skip all later actions");

        assertEquals(1, actions.get(), "a versioned migration action must execute at most once");
        assertEquals(2, persistence.reads,
            "each startup attempt must check the durable completion marker");
        assertEquals(1, persistence.writes,
            "only the first completed action may write the marker");
    }

    @Test
    void completedNoOpIsMarkedToPreserveLaterPlayerChoice() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);

        assertFalse(service.migrateOnce(MIGRATION_ID, () -> false, () -> {}),
            "an already-customized binding must remain a no-op");
        assertTrue(persistence.completed.contains(MIGRATION_ID),
            "a processed no-op must still be marked so a later intentional U is preserved");
    }

    @Test
    void persistenceReadAndWriteFailuresPropagateToTheLifecycleBoundary() {
        RecordingPersistence readFailure = new RecordingPersistence();
        readFailure.readFailure = new IllegalStateException("read failed");
        AtomicInteger readFailureActions = new AtomicInteger();

        IllegalStateException readException = assertThrows(IllegalStateException.class,
            () -> new KeybindMigrationService(readFailure).migrateOnce(MIGRATION_ID, () -> {
                readFailureActions.incrementAndGet();
                return true;
            }, () -> {}));
        assertEquals("read failed", readException.getMessage());
        assertEquals(0, readFailureActions.get(),
            "a marker read failure must not run an unguarded migration action");

        RecordingPersistence writeFailure = new RecordingPersistence();
        writeFailure.writeFailure = new IllegalStateException("write failed");
        AtomicInteger writeFailureActions = new AtomicInteger();
        AtomicInteger rollbacks = new AtomicInteger();

        IllegalStateException writeException = assertThrows(IllegalStateException.class,
            () -> new KeybindMigrationService(writeFailure).migrateOnce(MIGRATION_ID, () -> {
                writeFailureActions.incrementAndGet();
                return true;
            }, rollbacks::incrementAndGet));
        assertEquals("write failed", writeException.getMessage());
        assertEquals(1, writeFailureActions.get(),
            "a marker write failure occurs only after the migration action completes");
        assertEquals(1, rollbacks.get(),
            "a changed binding must be rolled back when its completion marker cannot be written");
        assertFalse(writeFailure.completed.contains(MIGRATION_ID),
            "a failed marker write must not appear durable to the next startup");
    }

    @Test
    void failedMarkerWriteAfterNoOpDoesNotRunRollback() {
        RecordingPersistence persistence = new RecordingPersistence();
        persistence.writeFailure = new IllegalStateException("write failed");
        AtomicInteger rollbacks = new AtomicInteger();

        IllegalStateException failure = assertThrows(IllegalStateException.class,
            () -> new KeybindMigrationService(persistence).migrateOnce(
                MIGRATION_ID,
                () -> false,
                rollbacks::incrementAndGet
            ));

        assertEquals("write failed", failure.getMessage());
        assertEquals(0, rollbacks.get(),
            "an already-customized binding must not be changed by rollback");
    }

    @Test
    void rollbackFailureIsSuppressedUnderTheMarkerWriteFailure() {
        RecordingPersistence persistence = new RecordingPersistence();
        persistence.writeFailure = new IllegalStateException("write failed");

        IllegalStateException failure = assertThrows(IllegalStateException.class,
            () -> new KeybindMigrationService(persistence).migrateOnce(
                MIGRATION_ID,
                () -> true,
                () -> {
                    throw new IllegalArgumentException("rollback failed");
                }
            ));

        assertEquals("write failed", failure.getMessage(),
            "the durable marker failure must remain the primary exception");
        assertEquals(1, failure.getSuppressed().length,
            "a rollback failure must remain observable without replacing the primary failure");
        assertEquals("rollback failed", failure.getSuppressed()[0].getMessage());
    }

    @Test
    void rolledBackMigrationCanRetryWithoutOverwritingALaterPlayerChoice() {
        RecordingPersistence persistence = new RecordingPersistence();
        persistence.writeFailure = new IllegalStateException("write failed");
        KeybindMigrationService service = new KeybindMigrationService(persistence);
        AtomicBoolean legacyKeyBound = new AtomicBoolean(true);

        assertThrows(IllegalStateException.class, () -> service.migrateOnce(
            MIGRATION_ID,
            () -> legacyKeyBound.compareAndSet(true, false),
            () -> legacyKeyBound.set(true)
        ));
        assertTrue(legacyKeyBound.get(),
            "a failed marker write must restore the legacy binding before startup continues");

        persistence.writeFailure = null;
        assertTrue(service.migrateOnce(
            MIGRATION_ID,
            () -> legacyKeyBound.compareAndSet(true, false),
            () -> legacyKeyBound.set(true)
        ), "the next startup must retry and durably complete the rolled-back migration");
        assertFalse(legacyKeyBound.get(), "the successful retry must apply the replacement binding");

        legacyKeyBound.set(true);
        assertFalse(service.migrateOnce(
            MIGRATION_ID,
            () -> legacyKeyBound.compareAndSet(true, false),
            () -> legacyKeyBound.set(true)
        ), "the durable marker must preserve a later intentional legacy-key choice");
        assertTrue(legacyKeyBound.get(),
            "a player choice made after completion must never be overwritten by this migration");
    }

    @Test
    void failedMigrationActionIsNotMarkedAndCanBeRetried() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);

        IllegalArgumentException failure = assertThrows(IllegalArgumentException.class,
            () -> service.migrateOnce(MIGRATION_ID, () -> {
                throw new IllegalArgumentException("migration failed");
            }, () -> {}));
        assertEquals("migration failed", failure.getMessage());
        assertEquals(0, persistence.writes,
            "a failed migration action must not write a completion marker");

        assertTrue(service.migrateOnce(MIGRATION_ID, () -> true, () -> {}),
            "an unmarked failed migration must remain retryable");
        assertEquals(1, persistence.writes,
            "the successful retry must write exactly one completion marker");
    }

    @Test
    void invalidDependenciesAndInputsFailAtTheServiceBoundary() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);

        assertThrows(NullPointerException.class, () -> new KeybindMigrationService(null));
        assertThrows(NullPointerException.class,
            () -> service.migrateOnce(null, () -> true, () -> {}));
        assertThrows(IllegalArgumentException.class,
            () -> service.migrateOnce(" ", () -> true, () -> {}));
        assertThrows(NullPointerException.class,
            () -> service.migrateOnce(MIGRATION_ID, null, () -> {}));
        assertThrows(NullPointerException.class,
            () -> service.migrateOnce(MIGRATION_ID, () -> true, null));
        assertEquals(0, persistence.reads,
            "invalid service inputs must fail before touching durable state");
    }

    private static final class RecordingPersistence implements KeybindMigrationPersistence {
        private final Set<String> completed = new HashSet<>();
        private IllegalStateException readFailure;
        private IllegalStateException writeFailure;
        private int reads;
        private int writes;

        @Override
        public boolean hasCompleted(String migrationId) {
            reads++;
            if (readFailure != null) {
                throw readFailure;
            }
            return completed.contains(migrationId);
        }

        @Override
        public void markCompleted(String migrationId) {
            writes++;
            if (writeFailure != null) {
                throw writeFailure;
            }
            completed.add(migrationId);
        }
    }
}
