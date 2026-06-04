package com.bong.client.coffin;

import com.bong.client.entity.BongEntityModelKind;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.EnumSource;

import static org.junit.jupiter.api.Assertions.assertFalse;
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
}
