package com.bong.client.audio;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcFootstepAudioControllerTest {
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
