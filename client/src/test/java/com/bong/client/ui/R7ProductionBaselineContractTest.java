package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class R7ProductionBaselineContractTest {
    @Test
    void productionSourceTreeMatchesFrozenBaseline() throws IOException {
        String[] fields = resourceLines().stream()
            .filter(R7SourceScan::isFixtureDataLine)
            .findFirst()
            .orElseThrow(() -> new AssertionError("missing production source baseline row"))
            .split("\\t", -1);
        assertEquals(3, fields.length, "malformed production source baseline row");
        assertEquals("SHA-256", fields[1], "production baseline must use the pinned digest algorithm");
        assertEquals(fields[2], R7SourceScan.sourceTreeDigest(R7SourceScan.productionRoot()),
            "production source tree drifted; update this baseline only with an intentional production change");
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7ProductionBaselineContractTest.class.getResourceAsStream(
            "/bong/ui/production-source-baseline.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing production source baseline fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }
}
