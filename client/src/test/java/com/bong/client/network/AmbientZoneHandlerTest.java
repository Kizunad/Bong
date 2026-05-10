package com.bong.client.network;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioLoopConfig;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.MusicStateMachine;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.audio.SoundSink;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AmbientZoneHandlerTest {
    @Test
    void parsesAmbientZonePayloadAndRoutesToMachine() {
        SoundRecipePlayer player = new SoundRecipePlayer(new NoopSink(), flag -> true);
        MusicStateMachine machine = new MusicStateMachine(player);
        AmbientZoneHandler handler = new AmbientZoneHandler(machine);
        String json = """
            {
              "v": 1,
              "zone_name": "spawn",
              "ambient_recipe_id": "ambient_spawn_plain",
              "music_state": "AMBIENT",
              "is_night": true,
              "season": "summer",
              "fade_ticks": 60,
              "pos": [0, 64, 0],
              "volume_mul": 1.5,
              "pitch_shift": 0.10,
              "recipe": {
                "id": "ambient_spawn_plain",
                "layers": [
                  { "sound": "minecraft:ambient.cave", "volume": 0.1, "pitch": 0.3, "delay_ticks": 0 }
                ],
                "loop": { "interval_ticks": 160, "while_flag": "audio_world" },
                "priority": 20,
                "attenuation": "zone_broadcast",
                "category": "AMBIENT"
              }
            }
            """;

        AmbientZoneHandler.RouteResult result = handler.route(json, json.getBytes().length);

        assertTrue(result.isHandled());
        assertEquals(MusicStateMachine.State.AMBIENT, machine.currentStateForTests());
        assertEquals(1, player.activeLoopCountForTests());
    }

    @Test
    void parserRejectsRecipeIdDrift() {
        String json = """
            {
              "v": 1,
              "zone_name": "spawn",
              "ambient_recipe_id": "ambient_spawn_plain",
              "music_state": "AMBIENT",
              "is_night": false,
              "season": "summer",
              "fade_ticks": 60,
              "volume_mul": 1.0,
              "pitch_shift": 0.0,
              "recipe": {
                "id": "combat_music",
                "layers": [
                  { "sound": "minecraft:ambient.cave", "volume": 0.1, "pitch": 1.0, "delay_ticks": 0 }
                ],
                "loop": { "interval_ticks": 160, "while_flag": "audio_world" },
                "priority": 20,
                "attenuation": "zone_broadcast",
                "category": "AMBIENT"
              }
            }
            """;

        AmbientZoneParseResult result = AmbientZoneHandler.parse(json, json.getBytes().length);

        assertTrue(!result.isSuccess());
    }

    @Test
    void parserRequiresSeason() {
        String json = validPayload().replace("  \"season\": \"summer\",\n", "");

        AmbientZoneParseResult result = AmbientZoneHandler.parse(json, json.getBytes().length);

        assertTrue(!result.isSuccess());
    }

    @Test
    void parserRejectsInvalidAmbientBounds() {
        String negativeFade = validPayload().replace("\"fade_ticks\": 60", "\"fade_ticks\": -1");
        String highVolume = validPayload().replace("\"volume_mul\": 1.5", "\"volume_mul\": 4.5");
        String highPitchShift = validPayload().replace("\"pitch_shift\": 0.10", "\"pitch_shift\": 1.5");

        assertTrue(!AmbientZoneHandler.parse(negativeFade, negativeFade.getBytes().length).isSuccess());
        assertTrue(!AmbientZoneHandler.parse(highVolume, highVolume.getBytes().length).isSuccess());
        assertTrue(!AmbientZoneHandler.parse(highPitchShift, highPitchShift.getBytes().length).isSuccess());
    }

    @Test
    void parserRejectsInvalidTsyDepth() {
        String nonStringDepth = validPayload().replace(
            "\"season\": \"summer\",",
            "\"season\": \"summer\",\n              \"tsy_depth\": 7,"
        );
        String unknownDepth = validPayload().replace(
            "\"season\": \"summer\",",
            "\"season\": \"summer\",\n              \"tsy_depth\": \"abyss\","
        );

        assertTrue(!AmbientZoneHandler.parse(nonStringDepth, nonStringDepth.getBytes().length).isSuccess());
        assertTrue(!AmbientZoneHandler.parse(unknownDepth, unknownDepth.getBytes().length).isSuccess());
    }

    private static String validPayload() {
        return """
            {
              "v": 1,
              "zone_name": "spawn",
              "ambient_recipe_id": "ambient_spawn_plain",
              "music_state": "AMBIENT",
              "is_night": true,
              "season": "summer",
              "fade_ticks": 60,
              "pos": [0, 64, 0],
              "volume_mul": 1.5,
              "pitch_shift": 0.10,
              "recipe": {
                "id": "ambient_spawn_plain",
                "layers": [
                  { "sound": "minecraft:ambient.cave", "volume": 0.1, "pitch": 0.3, "delay_ticks": 0 }
                ],
                "loop": { "interval_ticks": 160, "while_flag": "audio_world" },
                "priority": 20,
                "attenuation": "zone_broadcast",
                "category": "AMBIENT"
              }
            }
            """;
    }

    private static final class NoopSink implements SoundSink {
        @Override
        public boolean play(com.bong.client.audio.AudioScheduledSound sound) {
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
        }
    }
}
