package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.regex.Pattern;

/**
 * plan-skill-v1 §9 P2：保留老通道 {@code botany_skill} 做向后兼容（server 现有发送者尚未迁移）；
 * 收到 payload 后镜像到 {@link SkillSetStore} 的 {@link SkillId#HERBALISM} 条目，
 * 让 InspectScreen 技艺 tab 也能看到采药 skill。P7 将删除本 handler + 通道。
 */
public final class BotanySkillHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Integer level = readOptionalInteger(payload, "level");
        Long xp = readOptionalLong(payload, "xp");
        Long xpToNextLevel = readOptionalLong(payload, "xp_to_next_level");
        Integer autoUnlockLevel = readOptionalInteger(payload, "auto_unlock_level");
        if (level == null || xp == null || xpToNextLevel == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring botany_skill payload: required fields 'level', 'xp', or 'xp_to_next_level' are missing or invalid"
            );
        }

        // 镜像到 SkillSetStore 的 herbalism 条目，老通道不再独立持有 skill 状态。
        SkillSetSnapshot.Entry cur = SkillSetStore.snapshot().get(SkillId.HERBALISM);
        SkillSetSnapshot.Entry next = new SkillSetSnapshot.Entry(
            level,
            xp,
            xpToNextLevel,
            Math.max(cur.totalXp(), xp),
            cur.cap(),
            cur.recentGainXp(),
            cur.recentGainMillis()
        );
        SkillSetStore.updateEntry(SkillId.HERBALISM, next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied botany_skill level " + level + " to SkillSetStore (herbalism)"
        );
    }

    private static Integer readOptionalInteger(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            return null;
        }
        return Integer.parseInt(rawValue);
    }

    private static Long readOptionalLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            return null;
        }
        return Long.parseLong(rawValue);
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
