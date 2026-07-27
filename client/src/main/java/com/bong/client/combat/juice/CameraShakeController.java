package com.bong.client.combat.juice;

public final class CameraShakeController {
    public static final Offsets ZERO = new Offsets(0f, 0f);
    private static volatile Shake active = Shake.none();
    /** {@link #active} 的来源（所有权标记，与 {@link #active} 成对写）。 */
    private static volatile Source activeSource = Source.NONE;

    /**
     * 抖动幅度包络：
     * <ul>
     *   <li>{@link #DECAY}——命中「抖一下」线性前重衰减（1→0）；</li>
     *   <li>{@link #SUSTAIN}——施法 release「持续震动」（前 70% 满幅、末段收束）；</li>
     *   <li>{@link #CRESCENDO}——蓄力「渐强震动」（幅度 0→满，末段维持，撑到 release 顶替）。</li>
     * </ul>
     */
    public enum Envelope { DECAY, SUSTAIN, CRESCENDO }

    /**
     * 当前在播抖动的<b>来源</b>——本控制器是命中 juice 与施法 juice 共用的单通道
     *（last-write-wins），只靠 {@link #active} 分不出「现在震的是谁的」。
     *
     * <p>存在理由是**定向取消**：施法震感倍率调 0 只该停掉施法 juice 自己造的抖动，不该顺手
     * 掐掉一条正在播的命中抖动（玩家点的是「施法震感：关闭」，不是「相机别震了」）。
     * 死亡 / 断线 / 切世界的 {@link #clear()} 仍是无差别清场——那时整块画面都该安静。
     */
    public enum Source {
        /** 无在播抖动。 */
        NONE,
        /** 命中 / 格挡 / 击杀等战斗结算 juice（{@link #trigger} 与 {@link #triggerDirect}）。 */
        HIT,
        /** 施法 juice（{@code CastFovController}，走 {@link #triggerCast}）。 */
        CAST
    }

    private CameraShakeController() {
    }

    public static Shake trigger(CombatJuiceProfile profile, double directionX, double directionZ, long nowMs) {
        if (profile == null || profile.shakeIntensity() <= 0f || profile.shakeDurationTicks() <= 0) {
            return Shake.none();
        }
        return triggerDirect(
            profile.shakeIntensity(),
            profile.shakeDurationTicks(),
            directionX,
            directionZ,
            profile.reverseShake(),
            nowMs
        );
    }

    /**
     * plan-fpv-cast-av-v1 P3 —— 直接以显式强度/时长触发抖动（施法 release juice 用，其参数按招
     * pin 于 {@link CastJuiceProfile}，不经 school/tier 的 {@link CombatJuiceProfile}）。复用同一
     * {@link #active} 单通道（last-write-wins，与命中抖动共享，不新建相机通道）。
     * intensity ≤ 0 或 durationTicks ≤ 0 → 不触发（返回 {@link Shake#none()}）。
     * 本重载走 {@link Envelope#DECAY}，命中抖动用：线性前重衰减「抖一下」。
     */
    public static Shake triggerDirect(
        float intensity, int durationTicks, double directionX, double directionZ, boolean reverse, long nowMs
    ) {
        return triggerDirect(intensity, durationTicks, directionX, directionZ, reverse, Envelope.DECAY, nowMs);
    }

    /**
     * 显式包络重载：{@link Envelope#SUSTAIN} 施法 release 持续震动 / {@link Envelope#CRESCENDO}
     * 蓄力渐强震动 / {@link Envelope#DECAY} 命中抖一下。共享同一 {@link #active} 单通道
     * （last-write-wins——release 的 SUSTAIN 会顶替蓄力的 CRESCENDO）。
     */
    public static Shake triggerDirect(
        float intensity, int durationTicks, double directionX, double directionZ, boolean reverse,
        Envelope envelope, long nowMs
    ) {
        return trigger(intensity, durationTicks, directionX, directionZ, reverse, envelope,
            Source.HIT, nowMs);
    }

    /**
     * plan-fpv-cast-av-v1 P3 —— 施法 juice 的抖动入口：与命中抖动共用同一 {@link #active} 单通道
     * （last-write-wins 不变），但把来源登记成 {@link Source#CAST}，于是「施法震感倍率调 0」
     * 能只取消自己造的抖动（{@link #clearIfOwnedBy}），不误杀在播的命中抖动。
     */
    public static Shake triggerCast(
        float intensity, int durationTicks, double directionX, double directionZ,
        Envelope envelope, long nowMs
    ) {
        return trigger(intensity, durationTicks, directionX, directionZ, false, envelope,
            Source.CAST, nowMs);
    }

    private static Shake trigger(
        float intensity, int durationTicks, double directionX, double directionZ, boolean reverse,
        Envelope envelope, Source source, long nowMs
    ) {
        if (intensity <= 0f || durationTicks <= 0) {
            return Shake.none();
        }
        double[] perpendicular = perpendicular(directionX, directionZ, reverse);
        Shake next = new Shake(
            intensity,
            durationTicks,
            perpendicular[0],
            perpendicular[1],
            reverse,
            envelope == null ? Envelope.DECAY : envelope,
            Math.max(0L, nowMs)
        );
        // 先写来源再写抖动：读侧 activeOffsets 只看 active，所有权只被取消路径读，
        // 二者成对更新即可（两个字段都 volatile）。
        activeSource = source;
        active = next;
        return next;
    }

    /** 当前在播抖动的来源（{@link Source#NONE} = 无）。 */
    public static Source activeSource() {
        return activeSource;
    }

    public static Offsets activeOffsets(long nowMs) {
        Shake shake = active;
        if (shake == null || !shake.activeAt(nowMs)) {
            active = Shake.none();
            activeSource = Source.NONE;
            return ZERO;
        }
        return offsets(shake, nowMs);
    }

    public static Offsets offsets(Shake shake, long nowMs) {
        if (shake == null || !shake.activeAt(nowMs)) {
            return ZERO;
        }
        double ratio = shake.envelopeAt(nowMs);
        long elapsedTick = Math.max(0L, nowMs - shake.startedAtMs()) / 50L;
        double phase = switch ((int) (elapsedTick % 4L)) {
            case 0 -> 1.0;
            case 1 -> -0.85;
            case 2 -> 0.55;
            default -> -0.35;
        };
        double degrees = 2.0 * shake.intensity() * ratio * phase;
        return new Offsets((float) (shake.perpX() * degrees), (float) (Math.abs(shake.perpZ()) * degrees * 0.65));
    }

    public static double[] perpendicular(double directionX, double directionZ, boolean reverse) {
        if (!Double.isFinite(directionX)) {
            directionX = 0.0;
        }
        if (!Double.isFinite(directionZ)) {
            directionZ = 0.0;
        }
        double len = Math.sqrt(directionX * directionX + directionZ * directionZ);
        if (len <= 1e-6) {
            directionX = 0.0;
            directionZ = 1.0;
            len = 1.0;
        }
        double px = -directionZ / len;
        double pz = directionX / len;
        if (reverse) {
            px = -px;
            pz = -pz;
        }
        return new double[] { px, pz };
    }

    /** 死亡 / 断线 teardown：立即清空当前抖动（含蓄力 CRESCENDO），复位到无抖动。 */
    public static void clear() {
        active = Shake.none();
        activeSource = Source.NONE;
    }

    /**
     * <b>定向</b>取消：只在当前在播抖动确由 {@code owner} 创建时才清空，否则原样保留。
     *
     * <p>共用单通道的必要保护——施法震感倍率调 0 走这条，于是它不会顺手掐掉一条无关的命中
     * 抖动（{@link #clear()} 那种无差别清场只属于死亡 / 断线 / 切世界）。
     *
     * @return 是否真的清了
     */
    public static boolean clearIfOwnedBy(Source owner) {
        if (owner == null || owner == Source.NONE || activeSource != owner) {
            return false;
        }
        clear();
        return true;
    }

    public static void resetForTests() {
        active = Shake.none();
        activeSource = Source.NONE;
    }

    public record Shake(
        float intensity,
        int durationTicks,
        double perpX,
        double perpZ,
        boolean reverse,
        Envelope envelope,
        long startedAtMs
    ) {
        /** SUSTAIN 维持满幅的时间占比（前 70%），其后线性收束到 0。 */
        private static final double SUSTAIN_FRACTION = 0.7;
        /** CRESCENDO 幅度爬满的时间占比（前 90% 由 0 线性涨到满），其后维持满幅撑到 release。 */
        private static final double CRESCENDO_RAMP_FRACTION = 0.9;

        public static Shake none() {
            return new Shake(0f, 0, 0.0, 0.0, false, Envelope.DECAY, 0L);
        }

        public long durationMillis() {
            return Math.max(0, durationTicks) * 50L;
        }

        public boolean activeAt(long nowMs) {
            return intensity > 0f && durationMillis() > 0L && nowMs - startedAtMs < durationMillis();
        }

        /**
         * 幅度包络系数 ∈ [0,1]，按 {@link #envelope}：
         * {@link Envelope#DECAY} 线性衰减 1→0（抖一下）；{@link Envelope#SUSTAIN} 前
         * {@link #SUSTAIN_FRACTION} 满幅、末段收束（持续震动）；{@link Envelope#CRESCENDO}
         * 前 {@link #CRESCENDO_RAMP_FRACTION} 由 0 爬到满、其后维持满（蓄力渐强，撑到 release 顶替）。
         */
        public double envelopeAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0.0;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0.0;
            }
            double progress = elapsed / (double) duration;
            return switch (envelope) {
                case DECAY -> 1.0 - progress;
                case SUSTAIN -> progress <= SUSTAIN_FRACTION
                    ? 1.0
                    : 1.0 - (progress - SUSTAIN_FRACTION) / (1.0 - SUSTAIN_FRACTION);
                case CRESCENDO -> Math.min(1.0, progress / CRESCENDO_RAMP_FRACTION);
            };
        }

        public double remainingRatioAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0.0;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0.0;
            }
            return 1.0 - elapsed / (double) duration;
        }
    }

    public record Offsets(float yawDegrees, float pitchDegrees) {
        public boolean isZero() {
            return yawDegrees == 0f && pitchDegrees == 0f;
        }
    }
}
