package com.bong.client.inventory.model;

/**
 * plan-race-system-v1 PR-5b — client 侧「实体当前易形形态」快照条目
 * （与 Rust {@code MorphStateEntryV1} / proto {@code bong.MorphStateEntry} 精确对应）。
 *
 * <p>{@code modelKind} 现阶段服务端恒发 0（stub，未接线具体模型变体枚举）——渲染主键
 * **必须**用 {@code formRaceId}（当前仅 {@code "whale"} 一条真数据），不依赖
 * {@code modelKind}，见 {@link com.bong.client.morph.MorphModelRegistry}。
 *
 * <p>{@code formBodyPlanId} 随快照一并携带（供未来按 body_plan 而非 race 渲染的消费点
 * 使用），本 PR 渲染 mixin 只消费 {@code formRaceId}。
 */
public record MorphEntry(int modelKind, String formRaceId, String formBodyPlanId) {
}
