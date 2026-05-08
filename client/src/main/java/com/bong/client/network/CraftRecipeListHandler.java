package com.bong.client.network;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-craft-v1 P2 — `craft_recipe_list` 处理器：玩家上线 / 配方表刷新时把
 * 全表写入 {@link CraftStore}。
 */
public final class CraftRecipeListHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonElement recipesEl = payload.get("recipes");
        if (recipesEl == null || !recipesEl.isJsonArray()) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring craft_recipe_list: missing recipes array");
        }
        JsonArray arr = recipesEl.getAsJsonArray();
        List<CraftRecipe> parsed = new ArrayList<>(arr.size());
        for (JsonElement el : arr) {
            if (!el.isJsonObject()) continue;
            CraftRecipe recipe = parseRecipe(el.getAsJsonObject());
            if (recipe != null) parsed.add(recipe);
        }
        CraftStore.replaceRecipes(parsed);
        return ServerDataDispatch.handled(envelope.type(),
            "Applied craft_recipe_list snapshot (" + parsed.size() + " recipes)");
    }

    static CraftRecipe parseRecipe(JsonObject obj) {
        String id = readString(obj, "id");
        if (id == null || id.isEmpty()) return null;
        CraftCategory category = CraftCategory.fromWire(readString(obj, "category"));
        String displayName = readString(obj, "display_name");
        if (displayName == null) displayName = id;
        List<CraftRecipe.MaterialEntry> materials = parseMaterials(obj.get("materials"));
        double qiCost = readDouble(obj, "qi_cost");
        long timeTicks = readLong(obj, "time_ticks");
        JsonElement outputEl = obj.get("output");
        String outputTemplate = "";
        int outputCount = 0;
        if (outputEl != null && outputEl.isJsonArray()) {
            JsonArray outArr = outputEl.getAsJsonArray();
            if (outArr.size() == 2) {
                outputTemplate = outArr.get(0).getAsString();
                outputCount = outArr.get(1).getAsInt();
            }
        }
        CraftRecipe.Requirements req = parseRequirements(obj.get("requirements"));
        boolean unlocked = readBoolean(obj, "unlocked");
        return new CraftRecipe(id, category, displayName, materials, qiCost, timeTicks,
            outputTemplate, outputCount, req, unlocked);
    }

    private static List<CraftRecipe.MaterialEntry> parseMaterials(JsonElement el) {
        if (el == null || !el.isJsonArray()) return List.of();
        JsonArray arr = el.getAsJsonArray();
        List<CraftRecipe.MaterialEntry> out = new ArrayList<>(arr.size());
        for (JsonElement m : arr) {
            if (!m.isJsonArray()) continue;
            JsonArray mArr = m.getAsJsonArray();
            if (mArr.size() != 2) continue;
            String tpl = mArr.get(0).getAsString();
            int count = mArr.get(1).getAsInt();
            out.add(new CraftRecipe.MaterialEntry(tpl, count));
        }
        return out;
    }

    private static CraftRecipe.Requirements parseRequirements(JsonElement el) {
        if (el == null || !el.isJsonObject()) return CraftRecipe.Requirements.NONE;
        JsonObject obj = el.getAsJsonObject();
        String realmMin = readString(obj, "realm_min");
        String qiColorKind = null;
        Float qiColorShare = null;
        JsonElement colorEl = obj.get("qi_color_min");
        if (colorEl != null && colorEl.isJsonArray()) {
            JsonArray arr = colorEl.getAsJsonArray();
            if (arr.size() == 2) {
                qiColorKind = arr.get(0).getAsString();
                qiColorShare = arr.get(1).getAsFloat();
            }
        }
        Integer skillLvMin = null;
        JsonElement lvEl = obj.get("skill_lv_min");
        if (lvEl != null && lvEl.isJsonPrimitive() && lvEl.getAsJsonPrimitive().isNumber()) {
            skillLvMin = lvEl.getAsInt();
        }
        return new CraftRecipe.Requirements(realmMin, qiColorKind, qiColorShare, skillLvMin);
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static double readDouble(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        return el.getAsDouble();
    }

    private static long readLong(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0L;
        return el.getAsLong();
    }

    private static boolean readBoolean(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean()
            && el.getAsBoolean();
    }
}
