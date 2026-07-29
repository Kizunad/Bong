package com.bong.client.audio;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcFootstepAudioControllerTest {
    @BeforeEach
    @AfterEach
    void clearTrackedState() {
        NpcFootstepAudioController.clearOnDisconnect();
    }

    @Test
    void materialPlannerSelectsDistinctRecipes() {
        assertEquals("npc_footstep_default", NpcFootstepAudioController.recipeForMaterial("default").id());
        assertEquals("npc_footstep_ash", NpcFootstepAudioController.recipeForMaterial("ash").id());
        assertEquals("npc_footstep_water", NpcFootstepAudioController.recipeForMaterial("water").id());
    }

    @Test
    void npcFootstepUsesMeleeEnvironmentProfile() {
        AudioRecipe recipe = NpcFootstepAudioController.recipeForMaterial("ash");

        assertEquals(AudioAttenuation.MELEE, recipe.attenuation());
        assertEquals(AudioBus.ENVIRONMENT, recipe.bus());
    }

    @Test
    void firstNpcObservationOnlySeedsState() {
        NpcFootstepAudioController.StepDecision decision =
            NpcFootstepAudioController.planStep(null, 20, 1.0, 64.0, 2.0);

        assertFalse(decision.play());
        assertEquals(28, decision.next().nextTick());
    }

    @Test
    void disconnectClearDropsOldEntityStateAndFreshEntityCanBeTracked() {
        NpcFootstepAudioController.seedStateForTests(
            42,
            new NpcFootstepAudioController.StepState(1.0, 64.0, 2.0, 28L)
        );
        assertEquals(1, NpcFootstepAudioController.trackedStateCountForTests(),
            "old server entity step state must be present before disconnect cleanup");

        NpcFootstepAudioController.clearOnDisconnect();

        assertEquals(0, NpcFootstepAudioController.trackedStateCountForTests(),
            "entity-id state must not survive because a new world can reuse the same id");
        NpcFootstepAudioController.seedStateForTests(
            42,
            new NpcFootstepAudioController.StepState(8.0, 70.0, 9.0, 48L)
        );
        assertEquals(1, NpcFootstepAudioController.trackedStateCountForTests(),
            "fresh world state must be trackable after disconnect teardown");
    }

    @Test
    void npcStepRequiresIntervalAndMovementThreshold() {
        NpcFootstepAudioController.StepState previous =
            new NpcFootstepAudioController.StepState(1.0, 64.0, 2.0, 28);

        assertFalse(NpcFootstepAudioController.planStep(previous, 27, 2.0, 64.0, 2.0).play());
        assertFalse(NpcFootstepAudioController.planStep(previous, 28, 1.1, 64.0, 2.0).play());

        NpcFootstepAudioController.StepDecision decision =
            NpcFootstepAudioController.planStep(previous, 28, 1.3, 64.0, 2.0);
        assertTrue(decision.play());
        assertEquals(36, decision.next().nextTick());
    }
}
