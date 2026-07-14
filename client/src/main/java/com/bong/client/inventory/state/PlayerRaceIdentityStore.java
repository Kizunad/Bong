package com.bong.client.inventory.state;

/**
 * plan-race-system-v1 P3c — {@code cultivation_detail} 身份快照五字段
 * （{@code race_id}/{@code form_race_id}/{@code form_body_plan_id}/
 * {@code intrinsic_is_humanoid}/{@code form_is_humanoid}，见
 * {@code server/src/schema/server_data.rs} `CultivationDetail` 变体注释 §8.1
 * 决议 #5/#6）的 client 端权威缓存。
 *
 * <p>P3b 只把这五字段接到 server + TS schema，client Java 侧一直未解码/未消费——
 * 本 store 补上解码落点。装备 race gate 判定要求 **Form 身份**（易形后当前形态，
 * 未易形时 = 本体）：判 {@code Species} 用 {@link #formRaceId()}，判
 * {@code Humanoid} 用 {@link #formIsHumanoid()}；功法习得/施放门判**本体**用
 * {@link #raceId()} / {@link #intrinsicIsHumanoid()}——两者不同轴，别混用。
 *
 * <p>本 store 已接入装备槽 / 功法条目的主动置灰渲染：per-item {@code wearer_race}
 * 不走 {@code inventory_snapshot}（避免给该 payload 的 30+ 调用点串
 * {@code ItemRegistry} 依赖），而是由独立的 {@code race_gate_meta} payload
 * （{@link com.bong.client.network.RaceGateMetaHandler} → {@link RaceGateMetaStore}）
 * 下发 template_id/skill_id → {@code RaceGate} 映射表；{@link RaceGateEval} 联合本
 * store 的身份快照对每个装备槽 / 功法条目做纯判定，{@link com.bong.client.inventory.
 * component.EquipmentPanel}、{@link com.bong.client.inventory.InspectScreen}、
 * {@link com.bong.client.combat.inspect.TechniquesTabPanel} 据此置灰。装备被绕过
 * 提交时仍有 {@code race_mismatch} toast 兜底（见
 * {@link com.bong.client.network.InventoryMoveRejectedHandler}）——client 置灰只是
 * 预览，server 才是权威判定。
 */
public final class PlayerRaceIdentityStore {
    private static volatile String raceId = "";
    private static volatile String formRaceId = "";
    private static volatile String formBodyPlanId = "";
    private static volatile boolean intrinsicIsHumanoid;
    private static volatile boolean formIsHumanoid;

    private PlayerRaceIdentityStore() {
    }

    public static void replace(
        String raceId,
        String formRaceId,
        String formBodyPlanId,
        boolean intrinsicIsHumanoid,
        boolean formIsHumanoid
    ) {
        PlayerRaceIdentityStore.raceId = raceId == null ? "" : raceId;
        PlayerRaceIdentityStore.formRaceId = formRaceId == null ? "" : formRaceId;
        PlayerRaceIdentityStore.formBodyPlanId = formBodyPlanId == null ? "" : formBodyPlanId;
        PlayerRaceIdentityStore.intrinsicIsHumanoid = intrinsicIsHumanoid;
        PlayerRaceIdentityStore.formIsHumanoid = formIsHumanoid;
    }

    /** 本体种族 id（功法习得/施放门判定域）。 */
    public static String raceId() {
        return raceId;
    }

    /** 当前形态种族 id（装备门判定域；未易形时 = {@link #raceId()}）。 */
    public static String formRaceId() {
        return formRaceId;
    }

    /** 当前形态 body plan id（未易形时 = 本体 body_plan_id）。 */
    public static String formBodyPlanId() {
        return formBodyPlanId;
    }

    /** 本体是否人形。 */
    public static boolean intrinsicIsHumanoid() {
        return intrinsicIsHumanoid;
    }

    /** 当前形态是否人形（装备门判定域；未易形时 = {@link #intrinsicIsHumanoid()}）。 */
    public static boolean formIsHumanoid() {
        return formIsHumanoid;
    }

    /**
     * plan-race-system-v1 P3c fail-closed 判据：本体身份是否已收到权威快照。
     * server 生产恒发非空 {@code race_id}，空 = 尚未收到 cultivation_detail / 首帧乱序。
     * 功法门（判本体）在此为 false 时对**有 gate 的**功法一律置灰（宁可错灰不可错放）。
     */
    public static boolean intrinsicIdentityKnown() {
        return !raceId.isBlank();
    }

    /**
     * plan-race-system-v1 P3c fail-closed 判据：当前形态身份是否已收到权威快照。
     * 装备门（判当前形态）在此为 false 时对**有 gate 的**物品一律置灰。
     */
    public static boolean formIdentityKnown() {
        return !formRaceId.isBlank();
    }

    public static void resetForTests() {
        raceId = "";
        formRaceId = "";
        formBodyPlanId = "";
        intrinsicIsHumanoid = false;
        formIsHumanoid = false;
    }
}
