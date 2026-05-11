package com.bong.client.audio;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
}
