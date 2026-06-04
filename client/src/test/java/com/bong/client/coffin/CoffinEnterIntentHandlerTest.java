package com.bong.client.coffin;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
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
}
