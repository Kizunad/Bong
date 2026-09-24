package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7SemanticViewportContractTest {
    @Test
    void semanticSurfacePinsIdentityRevisionActionAndImmutableViewData() throws IOException {
        List<SemanticField> fields = readSemanticFields();
        assertEquals(12, fields.size(), "semantic surface fixture must enumerate every required field");
        assertEquals(fields.size(), new HashSet<>(fields.stream().map(SemanticField::field).toList()).size(),
            "semantic surface fields must be unique");
        Set<String> required = Set.of("surface_id", "template_id", "session_id", "revision", "view_data",
            "collection_identity", "action_id", "args_schema", "available");
        assertEquals(required, fields.stream().filter(SemanticField::required).map(SemanticField::field)
            .collect(java.util.stream.Collectors.toSet()),
            "semantic identity, state, and action fields must stay required as a set");
        assertTrue(fields.stream().allMatch(field -> !field.mutable()),
            "semantic surface must expose immutable values, never a mutable Store reference");
        assertTrue(fields.stream().allMatch(field -> !field.forbiddenShape().isBlank()),
            "every semantic field must pin at least one forbidden representation");
        assertFalse(fields.stream().anyMatch(field -> field.field().equals("available")
                && field.forbiddenShape().contains("server")),
            "available actions must not imply server acceptance before an authoritative receipt");
    }

    @Test
    void intentBoundaryPinsLayerOwnershipAndTransportOnlyResults() throws IOException {
        List<IntentBoundary> rows = readIntentBoundaries();
        assertEquals(6, rows.size(), "intent boundary fixture must enumerate every R7 layer");
        assertEquals(Set.of("contract", "state", "intent", "screen", "bootstrap", "headless"),
            rows.stream().map(IntentBoundary::layer).collect(java.util.stream.Collectors.toSet()),
            "intent boundary layers changed");
        assertTrue(rows.stream().filter(row -> row.layer().equals("contract"))
            .allMatch(row -> row.forbidden().contains("owo") && row.forbidden().contains("vanilla_widget")),
            "library-neutral contract cannot whitelist a concrete widget library");
        IntentBoundary intent = rows.stream().filter(row -> row.layer().equals("intent")).findFirst().orElseThrow();
        assertTrue(intent.allowed().contains("ClientRequestSender"),
            "typed intent adapters must be allowed to reuse the existing sender boundary");
        assertTrue(intent.forbidden().contains("ServerDataHandler")
                && intent.forbidden().contains("ProtoServerDataBridge")
                && intent.resultSemantics().contains("LOCAL_ACCEPTED"),
            "intent adapters must not become a second receive path and must return local transport results");
        IntentBoundary screen = rows.stream().filter(row -> row.layer().equals("screen")).findFirst().orElseThrow();
        assertTrue(screen.forbidden().contains("ClientRequestSender")
                && screen.forbidden().contains("ServerDataEnvelope"),
            "Screen code must not bypass the typed intent or state projection boundaries");
    }

    @Test
    void viewportMatrixCoversMinimumOddWideAndPortraitCases() throws IOException {
        List<ViewportCase> rows = readViewportCases();
        assertEquals(12, rows.size(), "viewport fixture must retain all supported geometry cases");
        assertEquals(12, rows.stream().map(ViewportCase::name).collect(Collectors.toSet()).size(),
            "viewport case names must be unique");
        assertTrue(rows.stream().allMatch(row -> row.logicalWidth() >= 320 && row.logicalHeight() >= 240),
            "every supported viewport must be at or above the frozen minimum 320x240");
        assertTrue(rows.stream().allMatch(row -> row.guiScales().equals("1,2,3,4")),
            "GUI scale coverage must remain 1-4 for every viewport");
        assertTrue(rows.stream().allMatch(row -> row.windowScales().equals("1.0,1.25,1.5,2.0")),
            "window scale coverage must remain 1.0/1.25/1.5/2.0 for every viewport");
        assertTrue(rows.stream().allMatch(row -> row.mode().equals("COMPACT_REGULAR_WIDE")),
            "layout policy must be expressed as a bounded mode, not an aspect-ratio special case");
        assertTrue(rows.stream().anyMatch(row -> row.name().equals("small_odd"))
                && rows.stream().anyMatch(row -> row.name().equals("ultrawide"))
                && rows.stream().anyMatch(row -> row.name().equals("portrait")),
            "matrix must include odd, ultrawide, and portrait geometry");
    }

    private static List<SemanticField> readSemanticFields() throws IOException {
        List<SemanticField> result = new ArrayList<>();
        for (String line : resourceLines("/bong/ui/semantic-surface.tsv")) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(6, fields.length, "malformed semantic surface row: " + line);
            result.add(new SemanticField(fields[0], fields[1], Boolean.parseBoolean(fields[2]),
                Boolean.parseBoolean(fields[3]), fields[4], fields[5]));
        }
        return result;
    }

    private static List<IntentBoundary> readIntentBoundaries() throws IOException {
        List<IntentBoundary> result = new ArrayList<>();
        for (String line : resourceLines("/bong/ui/intent-boundary.tsv")) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(5, fields.length, "malformed intent boundary row: " + line);
            result.add(new IntentBoundary(fields[0], fields[1], fields[2], fields[3], fields[4]));
        }
        return result;
    }

    private static List<ViewportCase> readViewportCases() throws IOException {
        List<ViewportCase> result = new ArrayList<>();
        for (String line : resourceLines("/bong/ui/viewport-matrix.tsv")) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(7, fields.length, "malformed viewport fixture row: " + line);
            result.add(new ViewportCase(fields[0], Integer.parseInt(fields[1]), Integer.parseInt(fields[2]),
                fields[3], fields[4], fields[5], Boolean.parseBoolean(fields[6])));
        }
        return result;
    }

    private static List<String> resourceLines(String resource) throws IOException {
        try (var stream = R7SemanticViewportContractTest.class.getResourceAsStream(resource)) {
            if (stream == null) {
                throw new AssertionError("missing R7 fixture: " + resource);
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record SemanticField(String field, String kind, boolean required, boolean mutable,
                                 String wireOwner, String forbiddenShape) {
    }

    private record IntentBoundary(String layer, String owner, String allowed, String forbidden,
                                  String resultSemantics) {
    }

    private record ViewportCase(String name, int logicalWidth, int logicalHeight, String guiScales,
                                String windowScales, String mode, boolean belowMinimum) {
    }

}
