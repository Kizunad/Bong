package com.bong.client.forge.state;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * F16 fix — {@code markDisplayed()}/{@code hasNewOutcome()}/{@code displayedOutcome}
 * were dead API (zero callers outside this class; only {@code lastOutcome()} is ever
 * read, by {@code ForgeProgressHudPlanner} and {@code ForgeScreen}). Removed the dead
 * surface and lock the surviving contract here so future edits don't silently regress it.
 */
class ForgeOutcomeStoreTest {
    @AfterEach
    void reset() {
        ForgeOutcomeStore.resetForTests();
    }

    @Test
    void defaultSnapshotIsEmptyWasteBucket() {
        ForgeOutcomeStore.Snapshot snapshot = ForgeOutcomeStore.lastOutcome();

        assertEquals(0, snapshot.sessionId());
        assertEquals("waste", snapshot.bucket());
        assertNull(snapshot.weaponItem());
    }

    @Test
    void replaceUpdatesLastOutcome() {
        ForgeOutcomeStore.Snapshot outcome = new ForgeOutcomeStore.Snapshot(
            7L, "blueprint_sword", "legendary", "iron_sword", 0.95f, "gold", "dizzy", 3, false
        );

        ForgeOutcomeStore.replace(outcome);

        assertEquals(outcome, ForgeOutcomeStore.lastOutcome());
    }

    @Test
    void replaceWithNullFallsBackToEmptySnapshot() {
        ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(
            1L, "bp", "rare", "sword", 0.5f, "blue", "", 1, false
        ));

        ForgeOutcomeStore.replace(null);

        assertEquals(ForgeOutcomeStore.Snapshot.empty(), ForgeOutcomeStore.lastOutcome());
    }

    @Test
    void resetForTestsClearsToEmptySnapshot() {
        ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(
            9L, "bp", "flawed", "axe", 0.2f, "red", "cracked", 1, true
        ));

        ForgeOutcomeStore.resetForTests();

        ForgeOutcomeStore.Snapshot snapshot = ForgeOutcomeStore.lastOutcome();
        assertEquals(0, snapshot.sessionId());
        assertEquals("waste", snapshot.bucket());
        assertFalse(snapshot.flawedPath());
    }
}
