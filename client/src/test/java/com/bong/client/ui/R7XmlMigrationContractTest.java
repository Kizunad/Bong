package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * R7 目标宿主门禁：所有 production Screen 最终统一为 owo XML。
 *
 * <p>当前实现仍在迁移中，因此 fixture 同时保存迁移前宿主和目标宿主，
 * 防止重构过程中漏掉 vanilla Screen 或 Java 构建的 owo Screen。</p>
 */
class R7XmlMigrationContractTest {
    @Test
    void everyProductionScreenHasAnExplicitOwoXmlMigrationTarget() throws IOException {
        List<MigrationRow> rows = resourceLines().stream()
            .filter(R7SourceScan::isFixtureDataLine)
            .map(R7XmlMigrationContractTest::parse)
            .toList();

        assertEquals(29, rows.size(), "XML migration inventory must cover every direct production Screen");
        assertEquals(29, rows.stream().map(MigrationRow::path).distinct().count(),
            "each production Screen must have exactly one migration row");
        assertTrue(rows.stream().allMatch(row -> row.targetHost().equals("OWO")
                && row.targetStyle().equals("OWO_XML_TEMPLATE")),
            "the post-migration production target must be owo XML for every Screen");
        assertEquals(12, rows.stream().filter(row -> row.currentHost().equals("VANILLA")).count(),
            "the migration inventory must account for all remaining vanilla Screens");
        assertEquals(12, rows.stream().filter(row -> row.currentHost().equals("OWO")
                && row.currentStyle().equals("CODE")).count(),
            "the migration inventory must account for the remaining 12 owo code-built Screens");
        assertEquals(2, rows.stream().filter(row -> row.currentHost().equals("OWO")
                && row.currentStyle().equals("XML_MODEL")).count(),
            "the two existing XML-based owo Screens must remain explicitly covered");
        assertEquals(3, rows.stream().filter(row -> row.currentHost().equals("OWO")
                && row.currentStyle().equals("OWO_XML_TEMPLATE")).count(),
            "已迁移的 owo XML Screen 必须在 inventory 中显式登记");
        assertEquals(Map.of("REWRITE_VANILLA", 12L, "REWRITE_OWO_CODE", 12L,
                "NORMALIZE_XML_TEMPLATE", 3L, "COMPLETE", 2L),
            rows.stream().collect(Collectors.groupingBy(MigrationRow::status, Collectors.counting())),
            "migration status counts must match the current Screen audit");
    }

    @Test
    void migrationRowsMatchThePreMigrationAdapterInventory() throws IOException {
        Map<String, MigrationRow> migration = resourceLines().stream()
            .filter(R7SourceScan::isFixtureDataLine)
            .map(R7XmlMigrationContractTest::parse)
            .collect(Collectors.toMap(MigrationRow::path, Function.identity()));
        Map<String, AdapterRow> adapters = adapterLines().stream()
            .filter(R7SourceScan::isFixtureDataLine)
            .map(R7XmlMigrationContractTest::parseAdapter)
            .collect(Collectors.toMap(AdapterRow::path, Function.identity()));

        assertEquals(adapters.keySet(), migration.keySet(),
            "XML migration rows must not invent or omit a production Screen");
        for (String path : adapters.keySet()) {
            AdapterRow adapter = adapters.get(path);
            MigrationRow row = migration.get(path);
            assertEquals(adapter.host(), row.currentHost(), "current host drifted for " + path);
            assertEquals(adapter.adapterStyle(), row.currentStyle(), "current style drifted for " + path);
        }
    }

    private static MigrationRow parse(String line) {
        String[] fields = line.split("\\t", -1);
        assertEquals(7, fields.length, "malformed XML migration row: " + line);
        return new MigrationRow(fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6]);
    }

    private static AdapterRow parseAdapter(String line) {
        String[] fields = line.split("\\t", -1);
        assertEquals(5, fields.length, "malformed adapter inventory row: " + line);
        return new AdapterRow(fields[0], fields[2], fields[3]);
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7XmlMigrationContractTest.class.getResourceAsStream("/bong/ui/ui-xml-migration.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing UI XML migration fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private static List<String> adapterLines() throws IOException {
        try (var stream = R7XmlMigrationContractTest.class.getResourceAsStream("/bong/ui/screen-adapters.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing Screen adapter fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record MigrationRow(String path, String className, String currentHost, String currentStyle,
                                String targetHost, String targetStyle, String status) {
    }

    private record AdapterRow(String path, String host, String adapterStyle) {
    }
}
