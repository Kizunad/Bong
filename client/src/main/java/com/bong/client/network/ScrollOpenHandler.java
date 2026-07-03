package com.bong.client.network;

import com.bong.client.scroll.ScrollOpenViewModel;
import com.bong.client.scroll.ScrollReadStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/**
 * 可阅读残卷阅读屏处理器（plan-scroll-reading-v1 P1）。
 *
 * <p>处理 {@code type="scroll_open"} 的 S2C payload（proto tag 138，§9），把
 * {@code {scroll_id, title, body_pages[]}} 合成 {@link ScrollOpenViewModel} 填入
 * {@link ScrollReadStore}。
 *
 * <p><b>可复用性验收</b>：本 handler 只读取 {@code ScrollOpen} 的三个契约字段，不 hardcode
 * 任何具体残卷内容——第二卷任意 {@code readable_scroll_spec} 挂的物品复用同一 payload
 * 形状即可，零 client 改动。
 *
 * <p>畸形 payload 守卫：{@code scroll_id} / {@code title} 缺失或为空白 → noOp；
 * {@code body_pages} 缺失、非数组、或过滤掉非字符串元素后为空 → noOp（"空页拒绝"）。
 */
public final class ScrollOpenHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        String scrollId = readString(payload, "scroll_id");
        if (scrollId == null || scrollId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "scroll_open: 'scroll_id' 缺失或为空，payload 忽略"
            );
        }

        String title = readString(payload, "title");
        if (title == null || title.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "scroll_open: 'title' 缺失或为空，payload 忽略（scroll_id=" + scrollId + "）"
            );
        }

        List<String> bodyPages = readBodyPages(payload);
        if (bodyPages.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "scroll_open: 'body_pages' 缺失/非法/空数组，payload 忽略（scroll_id=" + scrollId + "）"
            );
        }

        ScrollReadStore.replace(new ScrollOpenViewModel(scrollId, title, bodyPages));

        return ServerDataDispatch.handled(
            envelope.type(),
            "scroll_open 已推入 ScrollReadStore scroll_id='" + scrollId + "' pages=" + bodyPages.size()
        );
    }

    // ─── 解析 body_pages[] ────────────────────────────────────────────────

    private static List<String> readBodyPages(JsonObject payload) {
        JsonElement el = payload.get("body_pages");
        if (el == null || !el.isJsonArray()) {
            return List.of();
        }
        JsonArray array = el.getAsJsonArray();
        List<String> result = new ArrayList<>();
        for (JsonElement page : array) {
            if (page != null && page.isJsonPrimitive() && page.getAsJsonPrimitive().isString()) {
                result.add(page.getAsString());
            }
        }
        return result;
    }

    // ─── JSON 读取 helper ─────────────────────────────────────────────────

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isString()) {
            return null;
        }
        return el.getAsString();
    }
}
