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
 * <p>目前仅解码入 store，尚未接入装备槽主动置灰渲染——per-item {@code wearer_race}
 * 尚未下发到 client（{@code ItemTemplate.wearer_race} 是 server-only TOML 字段，接入
 * item wire 需给 {@code inventory_snapshot} 的 30+ 调用点串 {@code ItemRegistry}
 * 依赖，超出本次 P3c 范围；且当前 assets/items/*.toml 全部物品 wearer_race=Any，
 * 尚无真实种族门物品可验证）。装备被拒时仍有 {@code race_mismatch} toast 兜底
 * （见 {@link com.bong.client.network.InventoryMoveRejectedHandler}）。本 store
 * 是未来真正接入主动置灰时的现成数据源。
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

    public static void resetForTests() {
        raceId = "";
        formRaceId = "";
        formBodyPlanId = "";
        intrinsicIsHumanoid = false;
        formIsHumanoid = false;
    }
}
