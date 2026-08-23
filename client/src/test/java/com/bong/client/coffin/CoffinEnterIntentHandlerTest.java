package com.bong.client.coffin;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.ReservedInteractionIntents;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.EnumSource;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-coffin-tiers-v1 P3 — {@link CoffinEnterIntentHandler#isCoffinKind} truth-table tests.
 *
 * <p>延寿棺四档 MUNDANE/JADE/STONE/BRONZE → true；
 * 物资棺 COMMON/RARE/PRECIOUS 及其他 kind → false。
 */
class CoffinEnterIntentHandlerTest {

    // ─── 延寿棺四档应返 true ──────────────────────────────────────────────────

    @Test
    void mundaneCoffinIsRecognised() {
        assertTrue(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_MUNDANE),
            "COFFIN_MUNDANE should be a longevity-coffin kind"
        );
    }

    @Test
    void jadeCoffinIsRecognised() {
        assertTrue(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_JADE),
            "COFFIN_JADE should be a longevity-coffin kind"
        );
    }

    @Test
    void stoneCoffinIsRecognised() {
        assertTrue(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_STONE),
            "COFFIN_STONE should be a longevity-coffin kind"
        );
    }

    @Test
    void bronzeCoffinIsRecognised() {
        assertTrue(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_BRONZE),
            "COFFIN_BRONZE should be a longevity-coffin kind"
        );
    }

    // ─── 物资棺三档应返 false ────────────────────────────────────────────────

    @Test
    void commonSupplyCoffinIsRejected() {
        assertFalse(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_COMMON),
            "COFFIN_COMMON is a supply coffin, not a longevity coffin"
        );
    }

    @Test
    void rareSupplyCoffinIsRejected() {
        assertFalse(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_RARE),
            "COFFIN_RARE is a supply coffin, not a longevity coffin"
        );
    }

    @Test
    void preciousSupplyCoffinIsRejected() {
        assertFalse(
            CoffinEnterIntentHandler.isCoffinKind(BongEntityModelKind.COFFIN_PRECIOUS),
            "COFFIN_PRECIOUS is a supply coffin, not a longevity coffin"
        );
    }

    // ─── 其他无关 kind 全部返 false（穷举非棺 kind）──────────────────────────

    @ParameterizedTest(name = "{0} should not be a longevity-coffin kind")
    @EnumSource(
        value = BongEntityModelKind.class,
        names = {
            "COFFIN_MUNDANE", "COFFIN_JADE", "COFFIN_STONE", "COFFIN_BRONZE",
            "COFFIN_COMMON", "COFFIN_RARE", "COFFIN_PRECIOUS"
        },
        mode = EnumSource.Mode.EXCLUDE
    )
    void nonCoffinKindsAreRejected(BongEntityModelKind kind) {
        assertFalse(
            CoffinEnterIntentHandler.isCoffinKind(kind),
            kind + " must not pass isCoffinKind — only longevity coffins (MUNDANE/JADE/STONE/BRONZE) should"
        );
    }

    // ─── CodeRabbit major C: candidate() null-client + dispatch label-parsing ──

    /**
     * candidate(null) 应立即返回 Optional.empty()（防御 null client，无 NPE）。
     * 期望：entityHit(null) 在 client==null 分支返回 null，candidate 提前退出。
     */
    @Test
    void candidateReturnsEmptyWhenClientIsNull() {
        Optional<InteractCandidate> result =
            new CoffinEnterIntentHandler().candidate(null);
        assertFalse(
            result.isPresent(),
            "candidate(null) should return Optional.empty() because MinecraftClient is null; " +
            "expected empty but got: " + result
        );
    }

    // ─── candidateEntityId label-parsing（package-private 访问）──────────────

    /**
     * 有效 label "coffin_enter:42" → 解析 entity id = 42。
     * 期望：startsWith("coffin_enter:") 通过，parseInt("42") = 42。
     */
    @Test
    void candidateEntityIdParsesValidLabel() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer, 10, 1.0, "coffin_enter:42"
        );
        int id = CoffinEnterIntentHandler.candidateEntityId(candidate);
        assertEquals(
            42, id,
            "candidateEntityId should parse id=42 from label 'coffin_enter:42'; " +
            "期望 42，实得 " + id
        );
    }

    /**
     * label 前缀不匹配（supply_coffin: 前缀）→ 返回 -1（不命中）。
     * 期望：startsWith("coffin_enter:") = false → return -1。
     */
    @Test
    void candidateEntityIdRejectsWrongPrefix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer, 10, 1.0, "supply_coffin:99"
        );
        int id = CoffinEnterIntentHandler.candidateEntityId(candidate);
        assertEquals(
            -1, id,
            "candidateEntityId should return -1 for non-coffin label 'supply_coffin:99'; " +
            "期望 -1（wrong prefix），实得 " + id
        );
    }

    /**
     * null candidate → 返回 -1（防御 NPE）。
     * 期望：candidate == null 分支提前返回 -1。
     */
    @Test
    void candidateEntityIdRejectsNullCandidate() {
        int id = CoffinEnterIntentHandler.candidateEntityId(null);
        assertEquals(
            -1, id,
            "candidateEntityId(null) should return -1 (null-safe guard); " +
            "期望 -1，实得 " + id
        );
    }

    /**
     * label 前缀正确但后缀为非数字 → 返回 -1（NumberFormatException 被捕获）。
     * 期望：parseInt 抛 NumberFormatException → catch → return -1。
     */
    @Test
    void candidateEntityIdRejectsNonNumericSuffix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer, 10, 1.0, "coffin_enter:not_a_number"
        );
        int id = CoffinEnterIntentHandler.candidateEntityId(candidate);
        assertEquals(
            -1, id,
            "candidateEntityId should return -1 when suffix is non-numeric 'not_a_number'; " +
            "期望 -1（NumberFormatException caught），实得 " + id
        );
    }

    // ─── review finding [1]：合法 marker 的候选门控正面按压 ───────────────────

    /**
     * 合法 marker（COFFIN_MUNDANE + 距离 36.0 内）不得让 candidate 返回 empty。
     * 背景：Python GAP11 场景是 server 端点镜像，测不到「candidate 对合法 marker 恒返
     * empty」的编译级变异；本正面用例在 Java 生产 gate（candidateForCoffin，candidate
     * 直接调用）上锁死。
     */
    @Test
    void validMundaneMarkerProducesCandidate() {
        Optional<InteractCandidate> result =
            CoffinEnterIntentHandler.candidateForCoffin(BongEntityModelKind.COFFIN_MUNDANE, 4.0, 42);
        assertTrue(
            result.isPresent(),
            "合法 marker（COFFIN_MUNDANE、d2=4.0）必须产出 OpenContainer candidate，" +
            "否则 G 键对合法 marker 打开不了菜单（review finding [1] 的合法候选空返回攻击面）"
        );
        assertEquals("coffin_enter:42", result.orElseThrow().debugLabel());
        assertEquals(InteractIntent.OpenContainer, result.orElseThrow().intent());
        assertEquals(
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            result.orElseThrow().priority(),
            "延寿棺 G 派发应走 OPEN_CONTAINER 优先级层"
        );
    }

    /**
     * 恰在交互半径边界（d2=36.0）的合法 marker 必须产出 candidate（`<=` 语义，非 `<`）。
     */
    @Test
    void candidateAtBoundaryDistanceSq36Present() {
        assertTrue(
            CoffinEnterIntentHandler
                .candidateForCoffin(BongEntityModelKind.COFFIN_JADE, 36.0, 7)
                .isPresent(),
            "d2=36.0 恰在 6 格边界时必须产出 candidate（MAX_INTERACT_DISTANCE_SQ 为 <= 语义）"
        );
    }

    /**
     * 距离略超边界（d2=37.0）必须返回 empty（off-by-one 拒收）。
     */
    @Test
    void candidateOutsideBoundaryDistanceSq37Empty() {
        assertFalse(
            CoffinEnterIntentHandler
                .candidateForCoffin(BongEntityModelKind.COFFIN_BRONZE, 37.0, 7)
                .isPresent(),
            "d2=37.0 超出 6 格边界必须返回 empty，否则远程也可 G 开菜单"
        );
    }

    /**
     * 物资棺 / null kind 不应产出 candidate（非延寿棺 gate）。
     */
    @Test
    void supplyCoffinAndNullKindProduceNoCandidate() {
        assertFalse(
            CoffinEnterIntentHandler
                .candidateForCoffin(BongEntityModelKind.COFFIN_COMMON, 1.0, 7)
                .isPresent(),
            "物资棺 COFFIN_COMMON 不属延寿棺，不得产出 G 菜单 candidate"
        );
        assertFalse(
            CoffinEnterIntentHandler.candidateForCoffin(null, 1.0, 7).isPresent(),
            "null kind 必须 fail-closed 返回 empty"
        );
    }
}
