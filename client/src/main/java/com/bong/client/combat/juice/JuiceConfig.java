package com.bong.client.combat.juice;

/**
 * plan-fpv-cast-av-v1 §P3「可及性」—— 施法 juice 强度总倍率的 <b>client 配置持有者</b>。
 *
 * <p><b>作用域 = 施法 juice</b>（玩家看到的名字就是「施法震感 / Cast Shake」，见
 * {@code JuiceControls.FEEDBACK_*_KEY} 与两份 lang）：「全局」指**一个开关管所有招**，不是
 * 「管全部 juice」。命中 / 格挡 / 击杀 juice（{@link CombatJuiceSystem}）走自己的 profile
 * 表，**不**受本倍率门控——把它们也接进来会改动既有命中手感，属另一份 plan 的交付面，需
 * owner 拍板后另开（plan §P3 已登记）。对应地，倍率调 0 只定向取消施法自己造的相机抖动
 *（{@link CameraShakeController#clearIfOwnedBy}），不掐在播的命中抖动。
 *
 * <p>形态对齐仓库既有 client 配置约定（{@code com.bong.client.combat.HudConfig}：静态持有者 +
 * setter + {@code resetToDefaults}）。倍率不再藏在 {@link CastFovController} 的私有静态字段里：
 * 配置值与消费它的控制器分离，运行时可调入口见 {@link JuiceControls}（keybind，先例
 * {@code HudImmersionControls} / {@code NpcInteractionLogControls}）。
 *
 * <p><b>持久化缺口（诚实标注，非「已完成」）</b>：本仓库 client 侧目前<b>没有任何</b>配置文件
 * 持久化层——无 {@code FabricLoader.getConfigDir()} 读写、无 owo config screen、无 ModMenu；
 * {@code HudConfig} 的类注释亦自陈其 tunable 是「for future wiring into the external config
 * file」。故本类只交<b>进程内配置持有者 + 默认值 + 运行时可调入口</b>，跨会话文件持久化
 * 归后续（plan §P3 已写明）。每次启动回到 {@link #DEFAULT_JUICE_MULTIPLIER}。
 *
 * <p><b>写入即传播（防接线孤岛）</b>：{@link #setJuiceMultiplier} 直接调
 * {@link CastFovController#onJuiceMultiplierChanged}，而不是「bootstrap 里注册 listener」——
 * 后者一旦漏注册就是静默孤岛（本仓库反复踩过），直连由编译期保证配置与控制器永不脱钩。
 */
public final class JuiceConfig {
    /** plan §P3「默认 1.0」。 */
    public static final float DEFAULT_JUICE_MULTIPLIER = 1.0f;
    /** 倍率上限：防手滑设成天文数字把画面震飞（0 = 关闭是下限，见 {@link #clamp}）。 */
    public static final float MAX_JUICE_MULTIPLIER = 2.0f;

    /**
     * {@link JuiceControls} keybind 的循环档位：关闭 → 半强 → 默认 → 加强。plan §P3 要求
     * 「0 = 关闭」必须是玩家可达的一档，故 0 排在表内而非仅靠 API。
     */
    private static final float[] CYCLE_LEVELS = { 0.0f, 0.5f, 1.0f, 1.5f };

    private static volatile float juiceMultiplier = DEFAULT_JUICE_MULTIPLIER;

    private JuiceConfig() {
    }

    /** 当前全局 juice 强度倍率（0 = 关闭）。 */
    public static float juiceMultiplier() {
        return juiceMultiplier;
    }

    /**
     * 设倍率并传播。NaN / ≤0 → 0（关闭）；超上限钳到 {@link #MAX_JUICE_MULTIPLIER}。
     *
     * <p>转 0 时 {@link CastFovController#onJuiceMultiplierChanged} 会把<b>进行中的</b> FOV 脉冲
     * 与相机抖动一并取消（plan §P3「进行中把倍率调 0 立即复位」），不是只影响后续脉冲。幂等。
     *
     * @return 实际生效（钳位后）的倍率
     */
    public static float setJuiceMultiplier(float value) {
        float next = clamp(value);
        juiceMultiplier = next;
        CastFovController.onJuiceMultiplierChanged(next);
        return next;
    }

    /**
     * 循环到下一档（{@link #CYCLE_LEVELS}），到顶回第一档。当前值不在档位表上（被 API 直接
     * 设成任意值）时取<b>严格大于</b>当前值的第一档，故任意起点都能走完整圈。
     *
     * @return 新倍率
     */
    public static float cycleJuiceMultiplier() {
        float current = juiceMultiplier;
        for (float level : CYCLE_LEVELS) {
            if (level > current + 1e-6f) {
                return setJuiceMultiplier(level);
            }
        }
        return setJuiceMultiplier(CYCLE_LEVELS[0]);
    }

    /** 档位表副本（测试 / UI 展示用，防外部改内部数组）。 */
    public static float[] cycleLevels() {
        return CYCLE_LEVELS.clone();
    }

    /** 回默认值（走 {@link #setJuiceMultiplier}，保证传播不被绕过）。 */
    public static void resetToDefaults() {
        setJuiceMultiplier(DEFAULT_JUICE_MULTIPLIER);
    }

    static void resetForTests() {
        juiceMultiplier = DEFAULT_JUICE_MULTIPLIER;
    }

    private static float clamp(float value) {
        if (Float.isNaN(value) || value <= 0f) {
            return 0f;
        }
        return Math.min(value, MAX_JUICE_MULTIPLIER);
    }
}
