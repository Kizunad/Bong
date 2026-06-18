package com.bong.client.network;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.cultivation.TechniqueObserveHud;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/** 处理功法熟练度增量推送。 */
public final class TechniqueProficiencyUpdateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String techniqueId = readString(payload, "technique_id");
        Float proficiency = readFloat(payload, "proficiency");
        Float gain = readFloat(payload, "gain");
        if (techniqueId == null || techniqueId.isBlank() || proficiency == null || gain == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring technique_proficiency_update: invalid or missing technique_id/proficiency/gain");
        }

        List<TechniquesListPanel.Technique> current = TechniquesListPanel.snapshot();
        if (current.isEmpty()) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring technique_proficiency_update: no techniques_snapshot applied yet");
        }

        List<TechniquesListPanel.Technique> next = new ArrayList<>(current.size());
        TechniquesListPanel.Technique updated = null;
        for (TechniquesListPanel.Technique technique : current) {
            if (!technique.id().equals(techniqueId)) {
                next.add(technique);
                continue;
            }
            updated = withProficiency(technique, proficiency);
            next.add(updated);
        }

        if (updated == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring technique_proficiency_update: unknown technique " + techniqueId);
        }

        TechniquesListPanel.replace(next);
        if (gain > 0.0f) {
            TechniqueObserveHud.showProficiencyUp(
                updated.displayName(),
                updated.proficiency(),
                System.currentTimeMillis()
            );
        }
        return ServerDataDispatch.handled(envelope.type(),
            "Applied technique_proficiency_update " + techniqueId + " proficiency=" + updated.proficiency());
    }

    private static TechniquesListPanel.Technique withProficiency(
        TechniquesListPanel.Technique technique,
        float proficiency
    ) {
        return new TechniquesListPanel.Technique(
            technique.id(),
            technique.displayName(),
            technique.aliases(),
            technique.grade(),
            proficiency,
            "",
            technique.active(),
            technique.castKey(),
            technique.description(),
            technique.requiredRealm(),
            technique.requiredMeridians(),
            technique.qiCost(),
            technique.castTicks(),
            technique.cooldownTicks(),
            technique.range()
        );
    }

    private static String readString(JsonObject obj, String fieldName) {
        JsonElement element = obj.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : null;
    }

    private static Float readFloat(JsonObject obj, String fieldName) {
        JsonElement element = obj.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        try {
            float value = primitive.getAsFloat();
            return Float.isFinite(value) ? value : null;
        } catch (NumberFormatException ignored) {
            return null;
        }
    }
}
