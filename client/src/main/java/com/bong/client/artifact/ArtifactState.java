package com.bong.client.artifact;

import com.bong.client.cultivation.ColorKind;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.List;
import java.util.Optional;

public final class ArtifactState {
    public static final String TAG_PREFIX = "artifact_state:";

    private final int grooveCount;
    private final double totalDepth;
    private final double depthCap;
    private final int qualityTier;
    private final int overloadCracks;
    private final double maxCrackSeverity;
    private final ColorKind mainColor;
    private final double colorWeightTotal;

    private ArtifactState(
        int grooveCount,
        double totalDepth,
        double depthCap,
        int qualityTier,
        int overloadCracks,
        double maxCrackSeverity,
        ColorKind mainColor,
        double colorWeightTotal
    ) {
        this.grooveCount = Math.max(0, grooveCount);
        this.totalDepth = Math.max(0.0, totalDepth);
        this.depthCap = Math.max(0.0, depthCap);
        this.qualityTier = Math.max(0, qualityTier);
        this.overloadCracks = Math.max(0, overloadCracks);
        this.maxCrackSeverity = clamp01(maxCrackSeverity);
        this.mainColor = colorWeightTotal > 0.0 ? mainColor : null;
        this.colorWeightTotal = Math.max(0.0, colorWeightTotal);
    }

    public static Optional<ArtifactState> fromSideEffects(List<String> sideEffects) {
        if (sideEffects == null || sideEffects.isEmpty()) {
            return Optional.empty();
        }
        for (String sideEffect : sideEffects) {
            if (!isArtifactTag(sideEffect)) continue;
            Optional<ArtifactState> parsed = parseTag(sideEffect);
            if (parsed.isPresent()) return parsed;
        }
        return Optional.empty();
    }

    public static boolean isArtifactTag(String tag) {
        return tag != null && tag.startsWith(TAG_PREFIX);
    }

    public int grooveCount() {
        return grooveCount;
    }

    public double totalDepth() {
        return totalDepth;
    }

    public double depthCap() {
        return depthCap;
    }

    public double averageDepth() {
        return grooveCount == 0 ? 0.0 : totalDepth / grooveCount;
    }

    public int qualityTier() {
        return qualityTier;
    }

    public int overloadCracks() {
        return overloadCracks;
    }

    public double maxCrackSeverity() {
        return maxCrackSeverity;
    }

    public double maturity() {
        return depthCap <= 0.0 ? 0.0 : clamp01(totalDepth / depthCap);
    }

    public double resonancePreview() {
        double colorFactor = colorWeightTotal <= 0.0 ? 0.5 : 1.0;
        double crackFactor = maxCrackSeverity >= 0.16 ? 0.8 : 1.0;
        return clamp01(maturity() * colorFactor * crackFactor);
    }

    public String mainColorLabel() {
        return mainColor == null ? "无色" : mainColor.label();
    }

    public int indicatorColor() {
        int base = mainColor == null ? 0xFF808080 : mainColor.argb();
        double brightness = 0.35 + 0.65 * resonancePreview();
        return scaleRgb(base, brightness);
    }

    public String crackLabel() {
        if (maxCrackSeverity >= 0.71) return "断裂";
        if (maxCrackSeverity >= 0.41) return "深裂";
        if (maxCrackSeverity >= 0.16) return "裂纹";
        if (maxCrackSeverity > 0.0) return "微裂";
        return "无";
    }

    private static Optional<ArtifactState> parseTag(String tag) {
        try {
            JsonElement parsed = JsonParser.parseString(tag.substring(TAG_PREFIX.length()));
            if (parsed == null || !parsed.isJsonObject()) return Optional.empty();
            JsonObject root = parsed.getAsJsonObject();
            JsonObject meridian = object(root, "meridian");
            if (meridian == null) return Optional.empty();

            JsonArray grooves = array(meridian, "grooves");
            int grooveCount = grooves == null ? readInt(meridian, "groove_count", 0) : grooves.size();
            double computedDepth = 0.0;
            double computedCap = 0.0;
            double maxCrack = 0.0;
            if (grooves != null) {
                for (JsonElement element : grooves) {
                    if (element == null || !element.isJsonObject()) continue;
                    JsonObject groove = element.getAsJsonObject();
                    computedDepth += readDouble(groove, "depth", 0.0);
                    computedCap += readDouble(groove, "depth_cap", 0.0);
                    maxCrack = Math.max(maxCrack, readDouble(groove, "crack_severity", 0.0));
                }
            }

            JsonObject color = object(root, "color");
            ColorKind main = color == null ? null : ColorKind.fromWire(readString(color, "main", ""));
            double colorTotal = color == null ? 0.0 : practiceWeightTotal(color);

            return Optional.of(new ArtifactState(
                grooveCount,
                readDouble(meridian, "total_depth", computedDepth),
                readDouble(meridian, "depth_cap", computedCap),
                readInt(meridian, "quality_tier", 0),
                readInt(meridian, "overload_cracks", 0),
                maxCrack,
                main,
                colorTotal
            ));
        } catch (RuntimeException ignored) {
            return Optional.empty();
        }
    }

    private static double practiceWeightTotal(JsonObject color) {
        JsonObject practiceLog = object(color, "practice_log");
        JsonObject weights = practiceLog == null ? null : object(practiceLog, "weights");
        if (weights == null) return 0.0;
        double total = 0.0;
        for (String key : weights.keySet()) {
            total += readDouble(weights, key, 0.0);
        }
        return total;
    }

    private static JsonObject object(JsonObject object, String field) {
        JsonElement element = object.get(field);
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : null;
    }

    private static JsonArray array(JsonObject object, String field) {
        JsonElement element = object.get(field);
        return element != null && element.isJsonArray() ? element.getAsJsonArray() : null;
    }

    private static String readString(JsonObject object, String field, String fallback) {
        JsonElement element = object.get(field);
        if (element == null || !element.isJsonPrimitive()) return fallback;
        return element.getAsString();
    }

    private static int readInt(JsonObject object, String field, int fallback) {
        JsonElement element = object.get(field);
        if (element == null || !element.isJsonPrimitive()) return fallback;
        try {
            return element.getAsInt();
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static double readDouble(JsonObject object, String field, double fallback) {
        JsonElement element = object.get(field);
        if (element == null || !element.isJsonPrimitive()) return fallback;
        try {
            double value = element.getAsDouble();
            return Double.isFinite(value) ? value : fallback;
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static int scaleRgb(int argb, double factor) {
        int alpha = argb & 0xFF000000;
        int red = scaleChannel((argb >> 16) & 0xFF, factor);
        int green = scaleChannel((argb >> 8) & 0xFF, factor);
        int blue = scaleChannel(argb & 0xFF, factor);
        return alpha | (red << 16) | (green << 8) | blue;
    }

    private static int scaleChannel(int channel, double factor) {
        return Math.max(0, Math.min(255, (int) Math.round(channel * factor)));
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) return 0.0;
        return Math.max(0.0, Math.min(1.0, value));
    }
}
