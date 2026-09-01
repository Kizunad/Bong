package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7ScreenInventoryContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();
    private static final Pattern CLASS_EXTENDS = Pattern.compile(
        "\\bclass\\s+(\\w+)\\s+extends\\s+([^\\{\\n]+)");

    @Test
    void screenAdapterFixtureIsDerivedFromProductionDeclarations() throws IOException {
        List<ScreenAdapterRow> expected = readFixture();
        List<ScreenAdapterRow> actual = discoverDirectScreens();

        assertEquals(expected, actual,
            "R7 Screen adapter inventory drifted; update the fixture only after reviewing the new production declaration");
        assertEquals(29, actual.size(), "P0R must keep the current 29 direct Screen inventory entries");
        assertEquals(22, actual.stream().filter(row -> row.host().equals("OWO")).count(),
            "owo host count changed without an explicit migration decision");
        assertEquals(7, actual.stream().filter(row -> row.host().equals("VANILLA")).count(),
            "vanilla host count changed without an explicit migration decision");
        assertTrue(actual.stream().allMatch(row -> row.lifecycleOwner().equals("SCREEN_SCOPE")),
            "every adapter row must retain screen-local lifecycle ownership");
    }

    private static List<ScreenAdapterRow> discoverDirectScreens() throws IOException {
        List<ScreenAdapterRow> result = new ArrayList<>();
        try (var files = Files.walk(PRODUCTION_ROOT)) {
            for (Path path : files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList()) {
                String relative = PRODUCTION_ROOT.relativize(path).toString().replace('\\', '/');
                if (relative.startsWith("ui/adapter/owo/")) {
                    continue;
                }
                String source = R7SourceScan.read(path);
                String normalizedSource = source.replaceAll("\\s+", "");
                Matcher matcher = CLASS_EXTENDS.matcher(source);
                while (matcher.find()) {
                    String parent = matcher.group(2).replaceAll("\\s+", "");
                    String host;
                    String style;
                    if (parent.contains("BaseOwoScreen") || parent.contains("OwoXmlScreenHost")) {
                        host = "OWO";
                        style = parent.contains("OwoXmlScreenHost")
                            ? "OWO_XML_TEMPLATE"
                            : normalizedSource.contains("createAdapter(FlowLayout.class,this)")
                            ? "XML_MODEL"
                            : "CODE";
                        assertTrue(style.equals("OWO_XML_TEMPLATE")
                                || normalizedSource.contains("OwoUIAdapter.create")
                                || style.equals("XML_MODEL"),
                            "every direct owo Screen must expose one explicit adapter factory: " + path);
                    } else if (parent.equals("Screen") || parent.endsWith(".Screen")) {
                        host = "VANILLA";
                        style = "VANILLA";
                    } else {
                        continue;
                    }
                    result.add(new ScreenAdapterRow(
                        relative,
                        matcher.group(1),
                        host,
                        style,
                        "SCREEN_SCOPE"
                    ));
                }
            }
        }
        result.sort(Comparator.comparing(ScreenAdapterRow::path));
        return result;
    }

    private static List<ScreenAdapterRow> readFixture() throws IOException {
        List<ScreenAdapterRow> result = new ArrayList<>();
        for (String line : resourceLines()) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(5, fields.length, "malformed Screen adapter fixture row: " + line);
            result.add(new ScreenAdapterRow(fields[0], fields[1], fields[2], fields[3], fields[4]));
        }
        result.sort(Comparator.comparing(ScreenAdapterRow::path));
        return result;
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7ScreenInventoryContractTest.class.getResourceAsStream(
            "/bong/ui/screen-adapters.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing R7 Screen adapter fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record ScreenAdapterRow(String path, String className, String host,
                                    String adapterStyle, String lifecycleOwner) {
    }
}
