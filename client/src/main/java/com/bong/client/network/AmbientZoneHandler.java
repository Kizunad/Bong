package com.bong.client.network;

import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.MusicStateMachine;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Objects;
import java.util.Optional;

public final class AmbientZoneHandler {
    private final MusicStateMachine machine;

    public AmbientZoneHandler(MusicStateMachine machine) {
        this.machine = Objects.requireNonNull(machine, "machine");
    }

    public RouteResult route(String jsonPayload, int payloadSizeBytes) {
        AmbientZoneParseResult parseResult = parse(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult.errorMessage());
        }
        boolean changed = machine.apply(parseResult.payload().toUpdate());
        return changed
            ? RouteResult.handled(parseResult.payload())
            : RouteResult.noChange(parseResult.payload());
    }

    static AmbientZoneParseResult parse(String jsonPayload, int payloadSizeBytes) {
        JsonObject root = AudioEventEnvelope.parseRoot(jsonPayload, payloadSizeBytes);
        if (root == null) {
            return AmbientZoneParseResult.error("Malformed JSON: expected top-level object");
        }
        Integer version = AudioEventEnvelope.readRequiredInteger(root, "v");
        if (version == null || version != AudioEventEnvelope.EXPECTED_VERSION) {
            return AmbientZoneParseResult.error("Unsupported or missing version");
        }
        String zoneName = AudioEventEnvelope.readRequiredString(root, "zone_name");
        if (zoneName == null || zoneName.isBlank()) {
            return AmbientZoneParseResult.error("Invalid or missing zone_name");
        }
        String recipeId = AudioEventEnvelope.readRequiredString(root, "ambient_recipe_id");
        String musicStateRaw = AudioEventEnvelope.readRequiredString(root, "music_state");
        Optional<MusicStateMachine.State> state = MusicStateMachine.State.fromWire(musicStateRaw);
        if (recipeId == null || state.isEmpty()) {
            return AmbientZoneParseResult.error("Invalid recipe or music_state");
        }
        Boolean night = readBoolean(root, "is_night");
        String season = AudioEventEnvelope.readRequiredString(root, "season");
        Integer fadeTicks = AudioEventEnvelope.readRequiredInteger(root, "fade_ticks");
        Optional<AudioPosition> pos = AudioEventEnvelope.readOptionalPos(root, "pos");
        Float volumeMul = AudioEventEnvelope.readRequiredFloat(root, "volume_mul");
        Float pitchShift = AudioEventEnvelope.readRequiredFloat(root, "pitch_shift");
        AudioRecipe recipe = AudioEventEnvelope.parseRecipe(root.get("recipe"));
        if (night == null || fadeTicks == null || pos == null || volumeMul == null || pitchShift == null || recipe == null) {
            return AmbientZoneParseResult.error("Invalid ambient_zone payload fields");
        }
        if (!recipeId.equals(recipe.id())) {
            return AmbientZoneParseResult.error("ambient_recipe_id must equal recipe.id");
        }
        return AmbientZoneParseResult.success(new AmbientZonePayload(
            zoneName,
            recipeId,
            state.get(),
            night,
            season == null ? "" : season,
            readOptionalString(root, "tsy_depth"),
            fadeTicks,
            pos,
            volumeMul,
            pitchShift,
            recipe
        ));
    }

    private static Boolean readBoolean(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }

    private static Optional<String> readOptionalString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return Optional.empty();
        }
        if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString()) {
            return Optional.empty();
        }
        String value = element.getAsString().trim();
        return value.isEmpty() ? Optional.empty() : Optional.of(value);
    }

    public static final class RouteResult {
        private final Kind kind;
        private final String logMessage;

        private RouteResult(Kind kind, String logMessage) {
            this.kind = kind;
            this.logMessage = logMessage;
        }

        static RouteResult parseError(String logMessage) {
            return new RouteResult(Kind.PARSE_ERROR, logMessage);
        }

        static RouteResult handled(AmbientZonePayload payload) {
            return new RouteResult(Kind.HANDLED, "switched " + payload.debugDescriptor());
        }

        static RouteResult noChange(AmbientZonePayload payload) {
            return new RouteResult(Kind.NO_CHANGE, "kept " + payload.debugDescriptor());
        }

        public boolean isParseError() {
            return kind == Kind.PARSE_ERROR;
        }

        public boolean isHandled() {
            return kind == Kind.HANDLED;
        }

        public boolean isNoChange() {
            return kind == Kind.NO_CHANGE;
        }

        public String logMessage() {
            return logMessage;
        }

        public enum Kind {
            PARSE_ERROR,
            HANDLED,
            NO_CHANGE,
        }
    }
}
