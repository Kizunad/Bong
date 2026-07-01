package com.bong.client.compat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F14：{@link SodiumChunkReload} 版本守卫饱和单测。
 *
 * <p>Sodium 非 gradle 依赖（编译期不可见），headless 单测环境里
 * {@code Class.forName("me.jellysquid.mods.sodium...")} 恒抛 {@link ClassNotFoundException}——
 * 这正是绝大多数真实用户（未装 Sodium）的路径，也是本测试能覆盖到的分支：
 * <ul>
 *   <li>{@link SodiumChunkReload#register()} 检测不到 Sodium 时优雅降级（不抛异常，
 *       {@code sodiumPresent} 保持 false，不注册 tick 回调）</li>
 *   <li>{@link SodiumChunkReload#resolveSodiumVersion()}（本次新增，先例
 *       {@code BongIrisCompat.init()}）在 "sodium" mod 未加载时 fallback "unknown"，
 *       不抛异常</li>
 *   <li>失败阈值常量锁定（{@code CONSECUTIVE_FAILURE_DISABLE_THRESHOLD}）</li>
 * </ul>
 *
 * <p>反射 resync 成功/失败分支（需要 Sodium 真实在场）超出 headless 单测可达范围——
 * 这与修复前状态相同,不是本次回归引入的缺口,已在 F14 verify 结论里如实标注。
 */
class SodiumChunkReloadTest {

    @BeforeEach
    void setUp() {
        SodiumChunkReload.resetForTests();
    }

    @AfterEach
    void tearDown() {
        SodiumChunkReload.resetForTests();
    }

    // ---- register()：Sodium 缺席时优雅降级（真实用户默认路径） -----------------

    @Test
    void register_withoutSodiumOnClasspath_doesNotThrow() {
        assertDoesNotThrow(SodiumChunkReload::register,
            "Sodium 未安装时 register() 不应抛异常（绝大多数用户的默认路径）");
    }

    @Test
    void register_withoutSodiumOnClasspath_sodiumPresentStaysFalse() {
        SodiumChunkReload.register();
        assertFalse(SodiumChunkReload.isSodiumPresentForTests(),
            "headless 测试环境 Sodium 类不存在，register() 后 sodiumPresent 应为 false");
    }

    @Test
    void register_withoutSodiumOnClasspath_versionStaysUnknown() {
        // register() 在 ClassNotFoundException 分支提前 return，不应触碰版本探测逻辑，
        // sodiumVersion 保持 resetForTests() 设的初始值 "unknown"。
        SodiumChunkReload.register();
        assertEquals("unknown", SodiumChunkReload.sodiumVersionForTests(),
            "Sodium 不存在时不应进入版本探测分支，sodiumVersion 应保持初始 fallback");
    }

    @Test
    void register_calledTwice_remainsIdempotentAndSafe() {
        assertDoesNotThrow(() -> {
            SodiumChunkReload.register();
            SodiumChunkReload.register();
        }, "重复调用 register() 不应抛异常（防御式，虽然生产只调一次）");
    }

    // ---- resolveSodiumVersion()：F14 新增的版本探测,先例 BongIrisCompat ----------

    @Test
    void resolveSodiumVersion_whenSodiumModNotLoaded_returnsUnknown_doesNotThrow() {
        String version = assertDoesNotThrow(SodiumChunkReload::resolveSodiumVersion,
            "resolveSodiumVersion() 在 sodium mod 未加载时不应抛异常");
        assertNotNull(version, "version fallback 不应为 null");
        assertEquals("unknown", version,
            "测试环境未加载 sodium mod，resolveSodiumVersion() 应 fallback \"unknown\"");
    }

    @Test
    void resolveSodiumVersion_isPure_repeatedCallsReturnSameResult() {
        String first = SodiumChunkReload.resolveSodiumVersion();
        String second = SodiumChunkReload.resolveSodiumVersion();
        assertEquals(first, second,
            "同一环境下重复调用 resolveSodiumVersion() 应返回一致结果");
    }

    // ---- 失败阈值常量锁定 --------------------------------------------------------

    @Test
    void consecutiveFailureDisableThreshold_isPositiveAndSmall() {
        int threshold = SodiumChunkReload.consecutiveFailureDisableThresholdForTests();
        assertTrue(threshold > 0,
            "失败阈值必须为正数，否则第一次失败就会误触发禁用或永不禁用");
        assertTrue(threshold <= 20,
            "失败阈值应保持较小（<=20次，约 6.7 分钟轮询），避免 API 改名后长时间无意义刷日志; 实际="
                + threshold);
    }

    // ---- resetForTests：状态清零契约（供其他测试隔离用） ---------------------------

    @Test
    void resetForTests_clearsAllTrackedState() {
        SodiumChunkReload.register();

        SodiumChunkReload.resetForTests();

        assertFalse(SodiumChunkReload.isSodiumPresentForTests());
        assertEquals("unknown", SodiumChunkReload.sodiumVersionForTests());
        assertEquals(0, SodiumChunkReload.consecutiveFailuresForTests());
    }
}
