package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-race-system-v1 P0 review r3 (major x3 收口) -- Tests for
 * {@link MutationFeatureRenderer#resolveRenderable}, the pure (no-GL-context)
 * decision logic that resolves a wire {@code MutationSlotEntry} to a renderable
 * (kind, layout anchor) pair or an explicit "skip render" {@code null}. Covers
 * every real {@code BodySlot} variant, an unknown mutation kind, and a missing
 * layout mapping -- the two "残余调用者显式处理 None" branches called for by the
 * shared-contract review fix.
 */
class MutationFeatureRendererTest {

    private static final MutationSlotLayout DEFAULT_LAYOUT = MutationSlotLayoutRegistry.loadDefault();

    // ── every BodySlot variant resolves to its mapped part via a representative kind ──

    @Test
    void resolvesGoldenIrisOnHeadSlot() {
        var slot = new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNotNull(renderable);
        assertEquals(MutationKind.GOLDEN_IRIS, renderable.kind());
        assertEquals("head", renderable.layoutEntry().partId());
    }

    @Test
    void resolvesHardenedNailsOnForearmSlot() {
        var slot = new MutationVisualState.MutationSlotEntry("HardenedNails", "Forearm", 1);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNotNull(renderable);
        assertEquals(MutationKind.HARDENED_NAILS, renderable.kind());
        assertEquals("arm_r", renderable.layoutEntry().partId());
    }

    @Test
    void resolvesSpineSpursOnBackSlot() {
        var slot = new MutationVisualState.MutationSlotEntry("SpineSpurs", "Back", 2);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNotNull(renderable);
        assertEquals(MutationKind.SPINE_SPURS, renderable.kind());
        assertEquals("back", renderable.layoutEntry().partId());
    }

    @Test
    void resolvesBodyEnlargeOnTorsoSlot() {
        var slot = new MutationVisualState.MutationSlotEntry("BodyEnlarge", "Torso", 3);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNotNull(renderable);
        assertEquals(MutationKind.BODY_ENLARGE, renderable.kind());
        assertEquals("chest", renderable.layoutEntry().partId());
    }

    @Test
    void resolvesTailOnLowerSlot() {
        var slot = new MutationVisualState.MutationSlotEntry("Tail", "Lower", 2);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNotNull(renderable);
        assertEquals(MutationKind.TAIL, renderable.kind());
        assertEquals("abdomen", renderable.layoutEntry().partId());
    }

    // ── explicit "skip render" branches ──

    @Test
    void unknownMutationKindSkipsRenderRegardlessOfValidBodySlot() {
        var slot = new MutationVisualState.MutationSlotEntry("SomeFutureMutation", "Head", 1);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNull(renderable, "an unrecognized mutation kind must skip rendering, not fall back to a guess");
    }

    @Test
    void unknownBodySlotSkipsRenderRegardlessOfValidKind() {
        var slot = new MutationVisualState.MutationSlotEntry("GoldenIris", "Tentacle", 1);
        MutationFeatureRenderer.RenderableSlot renderable =
            MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT);
        assertNull(renderable, "a body_slot with no layout entry must skip rendering, not fall back to a guess");
    }

    @Test
    void missingMappingInASparseLayoutSkipsRenderForThatSlotOnly() {
        // Deliberately partial layout (as if a future non-humanoid body plan only
        // declares some slots) -- Forearm is intentionally absent.
        MutationSlotLayout sparseLayout = new MutationSlotLayout("test_partial", Map.of(
            "Head", new MutationSlotLayout.SlotEntry("head", MutationSlotLayout.Anchor.IDENTITY)
        ));

        var headSlot = new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1);
        assertNotNull(MutationFeatureRenderer.resolveRenderable(headSlot, sparseLayout),
            "Head is present in the sparse layout, must still resolve");

        var forearmSlot = new MutationVisualState.MutationSlotEntry("HardenedNails", "Forearm", 1);
        assertNull(MutationFeatureRenderer.resolveRenderable(forearmSlot, sparseLayout),
            "Forearm is absent from the sparse layout, must explicitly skip rather than reuse another slot's anchor");
    }

    @Test
    void nullKindStringSkipsRender() {
        var slot = new MutationVisualState.MutationSlotEntry(null, "Head", 1);
        assertNull(MutationFeatureRenderer.resolveRenderable(slot, DEFAULT_LAYOUT));
    }
}
