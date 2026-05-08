package com.bong.client.network;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/** Applies techniques_snapshot payloads to the learned-techniques store. */
public final class TechniquesSnapshotHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonArray entriesArr = SkillBarConfigHandler.readArray(envelope.payload(), "entries");
        if (entriesArr == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring techniques_snapshot payload: entries missing");
        }
        List<TechniquesListPanel.Technique> entries = new ArrayList<>();
        for (JsonElement el : entriesArr) {
            if (el == null || el.isJsonNull() || !el.isJsonObject()) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring techniques_snapshot payload: invalid entry");
            }
            TechniquesListPanel.Technique technique = parseTechnique(el.getAsJsonObject());
            if (technique == null) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring techniques_snapshot payload: malformed entry");
            }
            entries.add(technique);
        }
        TechniquesListPanel.replace(entries);
        return ServerDataDispatch.handled(envelope.type(), "Applied techniques_snapshot (" + entries.size() + " entries)");
    }

    private static TechniquesListPanel.Technique parseTechnique(JsonObject obj) {
        String id = SkillBarConfigHandler.readString(obj, "id");
        String displayName = SkillBarConfigHandler.readString(obj, "display_name");
        String grade = SkillBarConfigHandler.readString(obj, "grade");
        if (id == null || id.isEmpty() || displayName == null || displayName.isEmpty()) return null;
        List<String> aliases = parseAliases(obj);
        float proficiency = (float) readDouble(obj, "proficiency", 0.0);
        boolean active = readBoolean(obj, "active", false);
        String description = SkillBarConfigHandler.readString(obj, "description");
        String requiredRealm = SkillBarConfigHandler.readString(obj, "required_realm");
        float qiCost = (float) readDouble(obj, "qi_cost", 0.0);
        int castTicks = (int) SkillBarConfigHandler.readLong(obj, "cast_ticks", 0L);
        int cooldownTicks = (int) SkillBarConfigHandler.readLong(obj, "cooldown_ticks", 0L);
        float range = (float) readDouble(obj, "range", 0.0);
        List<TechniquesListPanel.RequiredMeridian> requiredMeridians = parseRequiredMeridians(obj);
        return new TechniquesListPanel.Technique(
            id,
            displayName,
            aliases,
            TechniquesListPanel.Grade.fromWire(grade),
            proficiency,
            active,
            "",
            description,
            requiredRealm,
            requiredMeridians,
            qiCost,
            castTicks,
            cooldownTicks,
            range
        );
    }

    private static List<String> parseAliases(JsonObject obj) {
        JsonArray arr = SkillBarConfigHandler.readArray(obj, "aliases");
        if (arr == null) return List.of();
        List<String> out = new ArrayList<>();
        for (JsonElement el : arr) {
            if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) continue;
            var primitive = el.getAsJsonPrimitive();
            if (!primitive.isString()) continue;
            String alias = primitive.getAsString();
            if (alias != null && !alias.isBlank()) out.add(alias);
        }
        return out;
    }

    private static List<TechniquesListPanel.RequiredMeridian> parseRequiredMeridians(JsonObject obj) {
        JsonArray arr = SkillBarConfigHandler.readArray(obj, "required_meridians");
        if (arr == null) return List.of();
        List<TechniquesListPanel.RequiredMeridian> out = new ArrayList<>();
        for (JsonElement el : arr) {
            if (el == null || el.isJsonNull() || !el.isJsonObject()) continue;
            JsonObject required = el.getAsJsonObject();
            String channel = SkillBarConfigHandler.readString(required, "channel");
            if (channel == null || channel.isBlank()) continue;
            out.add(new TechniquesListPanel.RequiredMeridian(
                channel,
                (float) readDouble(required, "min_health", 0.0)
            ));
        }
        return out;
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        var primitive = el.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : fallback;
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        var primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return fallback;
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }
}
