package com.bong.client.dying_elder;

/**
 * plan-dying-elder-v1 P3 — 垂死大能遭遇 HUD 全局状态存储。
 *
 * <p>由 {@link DyingElderEncounterHandler} 写入（S2C payload 解析后），
 * 由 {@link DyingElderHudPlanner} 读取生成 HUD 渲染指令。
 *
 * <p><b>守恒红线</b>：此存储仅持有 <em>显示层</em> 状态，
 * <strong>绝不</strong>修改玩家真元、血量或任何 gameplay 数值。
 *
 * <p>线程安全：{@code volatile} 字段保证渲染线程读取可见性；
 * 写入仅在 Minecraft 主线程（{@code client.execute()} 内）发生。
 */
public final class DyingElderEncounterStore {

    // ── 激活状态 ─────────────────────────────────────────────────────────────

    /** 是否当前有活跃的垂死大能遭遇（服务端已推送 appeared 事件）。 */
    private static volatile boolean active = false;

    // ── 大能身份 ─────────────────────────────────────────────────────────────

    /** 区域显示名称（用于 HUD 显示，例如 "坍缩渊"）。 */
    private static volatile String zoneDisplayName = "";

    /** 大能 MC protocol entity_id（Valence 从 1 起分配；0 = sentinel/未同步）。 */
    private static volatile int elderEntityId = 0;

    /** 最新的事件种类（"appeared" / "dan_received" / "betrayal" / "dead_natural" / "dead_player_kill"）。 */
    private static volatile String eventKind = "appeared";

    // ── 背叛概率（仅在 SpiritEye 激活时显示）────────────────────────────────

    /**
     * 背叛概率（0.0 ~ 1.0；{@code null} = 服务端尚未推送，不显示）。
     * 使用 Double 对象以区分"未知"与"0%"。
     */
    private static volatile Double betrayProbability = null;

    // ── 真元气息（SpiritEye 扫描到的大能真元比例）────────────────────────────

    /**
     * 大能当前真元占其最大值比例（0.0 ~ 1.0；0 = 未知/不可见）。
     * 由 SpiritEye 感知推送，用于渲染真元回复进度条。
     */
    private static volatile float qiFraction = 0.0f;

    // ── SpiritEye 状态 ───────────────────────────────────────────────────────

    /**
     * 玩家当前是否开着天目（SpiritEye）。
     * true 时 HUD 显示背叛概率；false 时显示"气息有异"或不显示。
     */
    private static volatile boolean spiritEyeActive = false;

    // ── 时间戳 ───────────────────────────────────────────────────────────────

    /** 最后一次接收到 elder_encounter payload 时服务端的 server_tick。 */
    private static volatile long receivedTick = 0L;

    private DyingElderEncounterStore() {}

    // ── 读取 API ─────────────────────────────────────────────────────────────

    /** 是否有活跃的垂死大能遭遇。 */
    public static boolean isActive() {
        return active;
    }

    /** 区域显示名称。 */
    public static String getZoneDisplayName() {
        return zoneDisplayName;
    }

    /** 大能 MC protocol entity_id（Valence 从 1 起分配；0 = sentinel/未同步）。 */
    public static int getElderEntityId() {
        return elderEntityId;
    }

    /** 最新事件种类字符串。 */
    public static String getEventKind() {
        return eventKind;
    }

    /**
     * 背叛概率（null = 未知）。
     * SpiritEye 未激活时 HUD 应显示"气息有异"而非具体数字。
     */
    public static Double getBetrayProbability() {
        return betrayProbability;
    }

    /** 大能真元比例（0.0 ~ 1.0）。 */
    public static float getQiFraction() {
        return qiFraction;
    }

    /** 玩家当前是否开着天目。 */
    public static boolean isSpiritEyeActive() {
        return spiritEyeActive;
    }

    /** 最后一次接收到 elder_encounter payload 的 server_tick。 */
    public static long getReceivedTick() {
        return receivedTick;
    }

    // ── 写入 API（仅供 Handler / 测试调用）──────────────────────────────────

    /**
     * 激活遭遇状态（收到 appeared 事件时调用）。
     *
     * @param zone             区域名称
     * @param protocolEntityId 大能 MC protocol entity_id（Valence 从 1 起分配；0 = sentinel）
     * @param tick             server_tick
     */
    public static void activate(String zone, int protocolEntityId, long tick) {
        zoneDisplayName = zone != null ? zone : "";
        elderEntityId = protocolEntityId;
        eventKind = "appeared";
        receivedTick = tick;
        active = true;
    }

    /**
     * 更新最新事件种类（dan_received / betrayal / dead_natural / dead_player_kill）。
     *
     * @param kind  事件种类字符串
     * @param tick  server_tick
     */
    public static void update(String kind, long tick) {
        if (kind != null) {
            eventKind = kind;
        }
        receivedTick = tick;
        // 死亡类事件后清理（保留短暂显示供玩家感知）
        boolean isDeath = "betrayal".equals(kind)
            || "dead_natural".equals(kind)
            || "dead_player_kill".equals(kind);
        if (isDeath) {
            active = false;
        }
    }

    /**
     * 更新背叛概率（SpiritEye 扫到大能后调用）。
     *
     * @param prob  背叛概率 0.0 ~ 1.0；null = 超出感知范围
     */
    public static void setBetrayProbability(Double prob) {
        betrayProbability = prob;
    }

    /**
     * 更新真元比例（SpiritEye 感知推送）。
     *
     * @param fraction 0.0 ~ 1.0
     */
    public static void setQiFraction(float fraction) {
        qiFraction = Math.max(0.0f, Math.min(1.0f, fraction));
    }

    /**
     * 更新 SpiritEye 激活状态。
     *
     * @param active true = 玩家已开天目
     */
    public static void setSpiritEyeActive(boolean active) {
        spiritEyeActive = active;
    }

    /**
     * 强制清除遭遇状态（断线 / 死亡 / 服务端主动关闭）。
     */
    public static void clear() {
        active = false;
        zoneDisplayName = "";
        elderEntityId = 0; // 0 = sentinel（Valence 从 1 起分配，0 表"无大能"）
        eventKind = "appeared";
        betrayProbability = null;
        qiFraction = 0.0f;
        spiritEyeActive = false;
        receivedTick = 0L;
    }

    /**
     * 断线清理（由 {@link com.bong.client.BongNetworkHandler} onDisconnect 调用）。
     */
    public static void clearOnDisconnect() {
        clear();
    }

    // ── 仅供测试的 accessor ──────────────────────────────────────────────────

    /** 仅供测试：完全重置。 */
    static void resetForTest() {
        clear();
    }

    /** 仅供测试：直接注入 active 状态。 */
    static void setActiveForTest(boolean value) {
        active = value;
    }

    /** 仅供测试：直接注入 qiFraction（不经 setQiFraction 的边界截断）。 */
    static void setQiFractionRawForTest(float value) {
        qiFraction = value;
    }
}
