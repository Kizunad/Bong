package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7BackendCapabilitiesContractTest {
    @Test
    void backendCapabilityFixturePinsOwoAsTheOnlyProductionHost() throws IOException {
        List<BackendRow> rows = resourceLines().stream()
            .filter(R7SourceScan::isFixtureDataLine)
            .map(R7BackendCapabilitiesContractTest::parse)
            .toList();
        assertEquals(3, rows.size(), "backend capability fixture must include owo, vanilla migration evidence, and one explicit out-of-scope decision");
        assertEquals(Set.of("OWO", "VANILLA", "THIRD_PARTY_HOST"),
            rows.stream().map(BackendRow::backend).collect(java.util.stream.Collectors.toSet()),
            "backend capability inventory drifted");
        BackendRow owo = rows.stream().filter(row -> row.backend().equals("OWO"))
            .findFirst().orElseThrow();
        assertEquals("AVAILABLE", owo.status(), "owo XML must remain the only production UI host");
        assertEquals("NONE", owo.fallback(), "owo production host must not fall back to another UI library");
        BackendRow vanilla = rows.stream().filter(row -> row.backend().equals("VANILLA"))
            .findFirst().orElseThrow();
        assertEquals("MIGRATION_ONLY", vanilla.status(),
            "vanilla is tracked only as pre-migration inventory, never as a production backend");
        assertEquals("OWO", vanilla.fallback(),
            "any remaining vanilla screen must migrate to the owo XML host");
        BackendRow thirdParty = rows.stream().filter(row -> row.backend().equals("THIRD_PARTY_HOST"))
            .findFirst().orElseThrow();
        assertEquals("OUT_OF_SCOPE", thirdParty.status(), "third host must remain explicitly out of scope");
        assertEquals("OWO", thirdParty.fallback(), "unsupported hosts must fall back to the sole production host");
        assertFalse(resourceLines().stream().anyMatch(line -> line.contains("MCEF") || line.contains("JCEF")
                || line.contains("CinemaMod") || line.contains("browser")),
            "R7 backend capability fixture must not reintroduce the retired browser-host route");
    }

    private static BackendRow parse(String line) {
        String[] fields = line.split("\\t", -1);
        assertEquals(6, fields.length, "malformed UI backend capability row: " + line);
        return new BackendRow(fields[0], fields[1], fields[2], fields[3], fields[4], fields[5]);
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7BackendCapabilitiesContractTest.class.getResourceAsStream(
            "/bong/ui/ui-backend-capabilities.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing UI backend capabilities fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record BackendRow(String backend, String mcVersion, String status,
                              String productionDependency, String fallback, String evidence) {
    }
}
