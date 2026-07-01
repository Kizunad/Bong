package com.bong.client.network;

import com.bong.client.coffin.TutorialCoffinPosStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import net.minecraft.util.math.BlockPos;

/**
 * F9 跨层修复 — 解析 server {@code tutorial_coffin_pos} payload，写入
 * {@link TutorialCoffinPosStore}。
 *
 * <p>Wire 形状（proto {@code TutorialCoffinPos}，经 {@link ProtoServerDataBridge} 展平后）：
 * <pre>{@code {"v":1,"type":"tutorial_coffin_pos","x":0,"y":69,"z":0}}</pre>
 * 三字段全部必须存在且为整数，否则视为异常 payload、不写入 store（保留旧值 / 空值，
 * 由 mixin 侧的 fallback 语义兜底，见
 * {@link com.bong.client.mixin.MixinClientPlayerInteractionManagerAlchemy}）。
 */
public final class TutorialCoffinPosHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Integer x = readInt(payload, "x");
        Integer y = readInt(payload, "y");
        Integer z = readInt(payload, "z");
        if (x == null || y == null || z == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring tutorial_coffin_pos payload: x/y/z missing or non-integer"
            );
        }

        TutorialCoffinPosStore.set(new BlockPos(x, y, z));
        return ServerDataDispatch.handled(
            envelope.type(),
            "tutorial_coffin_pos accepted (x=" + x + " y=" + y + " z=" + z + ")"
        );
    }

    private static Integer readInt(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        double raw = primitive.getAsDouble();
        if (!Double.isFinite(raw) || raw != Math.rint(raw)) {
            return null;
        }
        return (int) raw;
    }
}
