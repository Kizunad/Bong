package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-race-system-v1 P0 review r3 (major x3 收口) -- Tests for
 * {@link MutationSlotLayoutParser}, the parser for the shared
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} contract.
 */
class MutationSlotLayoutParserTest {

    private static final String VALID_JSON = """
        {
          "body_plan_id": "humanoid",
          "slots": {
            "Head": { "part_id": "head", "anchor": { "offset": [0.0, 1.65, 0.0], "scale": 1.031 } },
            "Forearm": { "part_id": "arm_r", "anchor": { "offset": [0.32, 1.2, 0.0], "scale": 1.032 } }
          }
        }
        """;

    @Test
    void parsesValidJsonWithBodyPlanIdAndSlots() {
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(VALID_JSON);
        assertTrue(result.ok(), "valid JSON must parse successfully, error=" + result.error());
        assertEquals("humanoid", result.layout().bodyPlanId());
    }

    @Test
    void parsesHeadSlotPartIdAndAnchor() {
        MutationSlotLayout layout = MutationSlotLayoutParser.parse(VALID_JSON).layout();
        MutationSlotLayout.SlotEntry head = layout.forBodySlot("Head");
        assertNotNull(head, "Head slot must parse");
        assertEquals("head", head.partId());
        assertEquals(0.0f, head.anchor().offsetX(), 0.0001f);
        assertEquals(1.65f, head.anchor().offsetY(), 0.0001f);
        assertEquals(0.0f, head.anchor().offsetZ(), 0.0001f);
        assertEquals(1.031f, head.anchor().scale(), 0.0001f);
    }

    @Test
    void parsesForearmSlotPartIdAndAnchor() {
        MutationSlotLayout layout = MutationSlotLayoutParser.parse(VALID_JSON).layout();
        MutationSlotLayout.SlotEntry forearm = layout.forBodySlot("Forearm");
        assertNotNull(forearm, "Forearm slot must parse");
        assertEquals("arm_r", forearm.partId());
        assertEquals(0.32f, forearm.anchor().offsetX(), 0.0001f);
        assertEquals(1.032f, forearm.anchor().scale(), 0.0001f);
    }

    @Test
    void unknownSlotKeyReturnsNullFromParsedLayout() {
        MutationSlotLayout layout = MutationSlotLayoutParser.parse(VALID_JSON).layout();
        assertNull(layout.forBodySlot("Back"), "slot not present in this JSON must resolve to null");
        assertNull(layout.forBodySlot("Tentacle"), "made-up slot name must resolve to null");
        assertNull(layout.forBodySlot(null), "null body_slot must resolve to null, not throw");
    }

    @Test
    void missingAnchorFallsBackToIdentity() {
        String json = """
            { "slots": { "Back": { "part_id": "back" } } }
            """;
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(json);
        assertTrue(result.ok(), "anchor is optional, missing anchor must still parse");
        MutationSlotLayout.SlotEntry back = result.layout().forBodySlot("Back");
        assertEquals("back", back.partId());
        assertEquals(MutationSlotLayout.Anchor.IDENTITY, back.anchor());
    }

    @Test
    void malformedJsonSyntaxReturnsError() {
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse("{ not valid json ][");
        assertFalse(result.ok());
        assertNull(result.layout());
        assertFalse(result.error().isBlank(), "error message must be populated for a parse failure");
    }

    @Test
    void missingSlotsKeyReturnsError() {
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse("{ \"body_plan_id\": \"humanoid\" }");
        assertFalse(result.ok(), "`slots` is required, must fail loudly rather than silently produce an empty layout");
    }

    @Test
    void slotEntryMissingPartIdReturnsError() {
        String json = """
            { "slots": { "Head": { "anchor": { "offset": [0,0,0], "scale": 1.0 } } } }
            """;
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(json);
        assertFalse(result.ok(), "a slot entry with no part_id must fail parsing, not silently default");
    }

    @Test
    void anchorOffsetWrongArityReturnsError() {
        String json = """
            { "slots": { "Head": { "part_id": "head", "anchor": { "offset": [0.0, 1.0], "scale": 1.0 } } } }
            """;
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(json);
        assertFalse(result.ok(), "anchor.offset with != 3 elements must fail parsing");
    }

    @Test
    void bodyPlanIdDefaultsToHumanoidWhenAbsent() {
        String json = """
            { "slots": { "Head": { "part_id": "head" } } }
            """;
        MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(json);
        assertTrue(result.ok());
        assertEquals("humanoid", result.layout().bodyPlanId());
    }
}
