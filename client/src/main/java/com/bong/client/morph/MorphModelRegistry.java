package com.bong.client.morph;

import java.util.Set;

/**
 * plan-race-system-v1 PR-5b — {@code form_race_id} → 「客户端有对应易形渲染模型」查询表。
 *
 * <p>{@code MorphStateEntryV1.model_kind} 服务端现阶段恒发 0（stub，未接线具体模型
 * 变体枚举），渲染主键**必须**用 {@code form_race_id} 字符串（当前仅
 * {@code "whale"} 一条真数据，对应 {@link com.bong.client.whale.WhaleRenderer} /
 * {@code whale.geo.json}），不依赖 {@code model_kind}。
 *
 * <p>查不到（{@link #hasModel} 返回 {@code false}）时渲染 mixin 必须放行走 vanilla
 * 玩家模型——未来新增易形目标种族但客户端资源包未跟上时的安全缺省（不能让玩家
 * 渲染成空白/崩溃）。
 */
public final class MorphModelRegistry {
    private static final Set<String> KNOWN_FORM_RACE_IDS = Set.of("whale");

    private MorphModelRegistry() {}

    public static boolean hasModel(String formRaceId) {
        return formRaceId != null && KNOWN_FORM_RACE_IDS.contains(formRaceId);
    }
}
