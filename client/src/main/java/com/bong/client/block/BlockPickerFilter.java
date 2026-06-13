package com.bong.client.block;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * plan-worldgen-v4 P5 §8.1#5 — dev 方块面板搜索过滤（pure，便于 headless 单测）。
 */
public final class BlockPickerFilter {
    /** 一次最多渲染的条目数，避免 ~900 vanilla 方块全量挂 owo 组件拖垮帧率。 */
    public static final int MAX_VISIBLE = 200;

    private BlockPickerFilter() {
    }

    /**
     * 大小写不敏感子串过滤，保持入参顺序，并截断到 {@link #MAX_VISIBLE}。
     *
     * @param blockIds 候选短名（已排序）
     * @param query    搜索串；null/空白返回全部（截断）
     */
    public static List<String> filter(List<String> blockIds, String query) {
        if (blockIds == null || blockIds.isEmpty()) {
            return List.of();
        }
        String needle = query == null ? "" : query.trim().toLowerCase(Locale.ROOT);
        List<String> out = new ArrayList<>();
        for (String id : blockIds) {
            if (id == null || id.isBlank()) {
                continue;
            }
            if (needle.isEmpty() || id.toLowerCase(Locale.ROOT).contains(needle)) {
                out.add(id);
                if (out.size() >= MAX_VISIBLE) {
                    break;
                }
            }
        }
        return List.copyOf(out);
    }
}
