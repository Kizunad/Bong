package com.bong.client.visual.realm_vision;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;

class RealmVisionFixtureTest {
    @Test
    void realmVisionFixtureRunsThroughReducerAndPlanner() throws IOException {
        JsonObject payload = JsonParser.parseString(
            java.nio.file.Files.readString(Path.of("..", "agent", "packages", "schema", "samples", "realm-vision-awaken.sample.json"))
        ).getAsJsonObject();
        RealmVisionState state = RealmVisionStateReducer.apply(RealmVisionState.empty(), payload, 0L);
        assertFalse(state.isEmpty());
        assertFalse(RealmVisionPlanner.plan(state, 0).fogEnd() < RealmVisionPlanner.plan(state, 0).fogStart());
    }
}
