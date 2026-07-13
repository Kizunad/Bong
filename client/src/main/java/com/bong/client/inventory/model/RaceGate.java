package com.bong.client.inventory.model;

import java.util.List;

/**
 * plan-race-system-v1 P3c — client 侧种族门（与 Rust {@code body_plan::types::RaceGateOwned}
 * / proto {@code bong.RaceGate} / TS {@code RaceGateV1} 精确对应）。
 *
 * <p>三档：
 * <ul>
 *   <li>{@code any}：任何身份放行（实际上 {@code RaceGateMeta} 表**不装** any 条目——
 *       client 查表 miss 即 any，此类型只在解码显式 any 时出现）。</li>
 *   <li>{@code humanoid}：判定域 {@code isHumanoid}。</li>
 *   <li>{@code species}：判定域 {@code raceId} 是否在 {@link #species()} 名单内。</li>
 * </ul>
 *
 * <p>判定域「本体 vs 当前形态」由**调用方**决定（装备门判当前形态、功法门判本体，
 * 决议 §8.1 #5/#6），本类型只做纯匹配，不关心传入的是哪套身份。
 *
 * <p>未知 {@code kind} 解码为 {@code null}（见 {@link #fromWire}）——调用方 fail-closed
 * 拒绝，不静默兜底 any：{@code RaceGateMetaHandler} 对该条目改存 {@link #blocked()}
 * 哨兵（而非跳过该条），确保 {@code RaceGateMetaStore} 查表命中一个恒拒绝的 gate，
 * 不会退化成"查不到 = any = 恒放行"的误判（plan-race-system-v1 P3 opus verify LOW 项）。
 */
public record RaceGate(String kind, List<String> species) {

    public static final String KIND_ANY = "any";
    public static final String KIND_HUMANOID = "humanoid";
    public static final String KIND_SPECIES = "species";
    /**
     * 哨兵 kind：server 下发了当前 client 版本不认识的未来 {@code gate.kind}。
     * 不属于三档合法 wire 值，命中 {@link #allows} 的 {@code default} 分支恒
     * 返回 {@code false}——即"永远阻断"，而不是被 {@code RaceGateMetaHandler}
     * 悄悄丢弃条目退化成 any 放行。
     */
    private static final String KIND_UNKNOWN_BLOCKED = "__unknown_blocked__";

    public RaceGate {
        kind = kind == null ? "" : kind;
        species = species == null ? List.of() : List.copyOf(species);
    }

    /**
     * 该门是否放行给定身份。判定域（本体 / 当前形态）由调用方选择传入。
     *
     * @param raceId     种族 id（species 档比对用）
     * @param isHumanoid 是否人形（humanoid 档比对用）
     */
    public boolean allows(String raceId, boolean isHumanoid) {
        return switch (kind) {
            case KIND_ANY -> true;
            case KIND_HUMANOID -> isHumanoid;
            case KIND_SPECIES -> raceId != null && species.contains(raceId);
            // 未知 kind 理论上不会走到（fromWire 已 fail-closed 成 null），防御性拒绝。
            default -> false;
        };
    }

    public boolean isAny() {
        return KIND_ANY.equals(kind);
    }

    /**
     * 从 wire {@code {kind, species}} 解码。未知 kind → {@code null}（fail-closed，
     * 调用方须拒绝，不兜底 any）。缺 kind / kind 非三档合法值均视作未知。
     */
    public static RaceGate fromWire(String kind, List<String> species) {
        if (kind == null) return null;
        return switch (kind) {
            case KIND_ANY, KIND_HUMANOID, KIND_SPECIES ->
                new RaceGate(kind, species == null ? List.of() : species);
            default -> null;
        };
    }

    /**
     * fail-closed 哨兵：未知 {@code gate.kind} 条目专用，{@link #allows} 恒
     * {@code false}。{@code RaceGateMetaHandler.parseTable} 对 {@link #fromWire}
     * 返回 {@code null} 的条目存入此哨兵而非跳过，保证该 id 在
     * {@code RaceGateMetaStore} 查表命中一个永远拒绝的 gate（不退化成"查不到 = any"）。
     */
    public static RaceGate blocked() {
        return new RaceGate(KIND_UNKNOWN_BLOCKED, List.of());
    }
}
