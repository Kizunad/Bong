package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.util.HashSet;
import java.util.Set;
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
        }), "the first successful migration must report that it changed the binding");
        assertFalse(service.migrateOnce(MIGRATION_ID, () -> {
            actions.incrementAndGet();
            return true;
        }), "a completed migration id must skip all later actions");

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

        assertFalse(service.migrateOnce(MIGRATION_ID, () -> false),
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
            }));
        assertEquals("read failed", readException.getMessage());
        assertEquals(0, readFailureActions.get(),
            "a marker read failure must not run an unguarded migration action");

        RecordingPersistence writeFailure = new RecordingPersistence();
        writeFailure.writeFailure = new IllegalStateException("write failed");
        AtomicInteger writeFailureActions = new AtomicInteger();

        IllegalStateException writeException = assertThrows(IllegalStateException.class,
            () -> new KeybindMigrationService(writeFailure).migrateOnce(MIGRATION_ID, () -> {
                writeFailureActions.incrementAndGet();
                return true;
            }));
        assertEquals("write failed", writeException.getMessage());
        assertEquals(1, writeFailureActions.get(),
            "a marker write failure occurs only after the migration action completes");
        assertFalse(writeFailure.completed.contains(MIGRATION_ID),
            "a failed marker write must not appear durable to the next startup");
    }

    @Test
    void failedMigrationActionIsNotMarkedAndCanBeRetried() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);

        IllegalArgumentException failure = assertThrows(IllegalArgumentException.class,
            () -> service.migrateOnce(MIGRATION_ID, () -> {
                throw new IllegalArgumentException("migration failed");
            }));
        assertEquals("migration failed", failure.getMessage());
        assertEquals(0, persistence.writes,
            "a failed migration action must not write a completion marker");

        assertTrue(service.migrateOnce(MIGRATION_ID, () -> true),
            "an unmarked failed migration must remain retryable");
        assertEquals(1, persistence.writes,
            "the successful retry must write exactly one completion marker");
    }

    @Test
    void invalidDependenciesAndInputsFailAtTheServiceBoundary() {
        RecordingPersistence persistence = new RecordingPersistence();
        KeybindMigrationService service = new KeybindMigrationService(persistence);

        assertThrows(NullPointerException.class, () -> new KeybindMigrationService(null));
        assertThrows(NullPointerException.class, () -> service.migrateOnce(null, () -> true));
        assertThrows(IllegalArgumentException.class, () -> service.migrateOnce(" ", () -> true));
        assertThrows(NullPointerException.class, () -> service.migrateOnce(MIGRATION_ID, null));
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
