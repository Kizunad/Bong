package com.bong.client.ui.adapter.owo;

import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.InputStream;
import java.net.JarURLConnection;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Enumeration;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.jar.JarFile;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

class OwoXmlTemplateRegistryTest {
    private static final String RESOURCE_ROOT = "assets/bong/owo_ui";

    @Test
    void registeredTemplatesArePackagedAndParsable() throws Exception {
        var registry = OwoXmlTemplateRegistry.production();
        Set<String> registeredPaths = new TreeSet<>();
        assertFalse(registry.templateIds().isEmpty(), "生产 owo 模板注册表不应为空");
        for (String template : registry.templateIds()) {
            var id = registry.identifierFor(template);
            assertTrue(registeredPaths.add(id.getPath()), "多个模板注册项指向同一个 XML: " + id);
            String resource = "/assets/" + id.getNamespace() + "/owo_ui/" + id.getPath() + ".xml";
            try (InputStream stream = getClass().getResourceAsStream(resource)) {
                assertNotNull(stream, "缺少随包发布的 owo XML: " + resource);
                assertNotNull(UIModel.load(stream), "owo 无法解析本地 XML: " + resource);
            }
        }
        Set<String> packagedPaths = packagedTemplatePaths();
        assertEquals(registeredPaths, packagedPaths,
            "注册表与随包 XML 必须双向一致：注册项和资源文件不能单边存在");
    }

    private static Set<String> packagedTemplatePaths() throws Exception {
        Set<String> paths = new TreeSet<>();
        Enumeration<URL> roots = OwoXmlTemplateRegistryTest.class.getClassLoader().getResources(RESOURCE_ROOT);
        assertTrue(roots.hasMoreElements(), "找不到随包发布的 owo XML 资源目录: " + RESOURCE_ROOT);
        while (roots.hasMoreElements()) {
            URL root = roots.nextElement();
            if ("file".equals(root.getProtocol())) {
                collectFilePaths(Path.of(root.toURI()), paths);
            } else if ("jar".equals(root.getProtocol())) {
                collectJarPaths((JarURLConnection) root.openConnection(), paths);
            } else {
                fail("不支持扫描 owo XML 资源目录的 URL 协议: " + root);
            }
        }
        return paths;
    }

    private static void collectFilePaths(Path root, Set<String> paths) throws Exception {
        try (Stream<Path> files = Files.walk(root)) {
            files.filter(Files::isRegularFile)
                .map(root::relativize)
                .map(Path::toString)
                .map(path -> path.replace(java.io.File.separatorChar, '/'))
                .filter(path -> path.endsWith(".xml"))
                .map(path -> path.substring(0, path.length() - ".xml".length()))
                .forEach(paths::add);
        }
    }

    private static void collectJarPaths(JarURLConnection connection, Set<String> paths) throws Exception {
        connection.setUseCaches(false);
        String prefix = RESOURCE_ROOT + "/";
        try (JarFile jar = connection.getJarFile()) {
            jar.stream()
                .map(java.util.jar.JarEntry::getName)
                .filter(path -> path.startsWith(prefix) && path.endsWith(".xml"))
                .map(path -> path.substring(prefix.length(), path.length() - ".xml".length()))
                .forEach(paths::add);
        }
    }

    @Test
    void unknownBlankAndNullTemplateIdsAreRejected() {
        OwoXmlTemplateRegistry registry = new OwoXmlTemplateRegistry(ignored -> null, Map.of());
        assertThrows(IllegalArgumentException.class, () -> registry.require("unknown"));
        assertThrows(IllegalArgumentException.class, () -> registry.require("  "));
        assertThrows(NullPointerException.class, () -> registry.require(null));
    }

    @Test
    void registeredButMissingResourceFailsFast() {
        Identifier missing = new Identifier("bong", "missing");
        OwoXmlTemplateRegistry registry = new OwoXmlTemplateRegistry(ignored -> null, Map.of("missing", missing));
        IllegalStateException failure = assertThrows(IllegalStateException.class, () -> registry.require("missing"));
        assertTrue(failure.getMessage().contains("bong:missing"));
    }

    @Test
    void constructorRejectsMalformedRegistryEntries() {
        Identifier id = new Identifier("bong", "valid");
        assertThrows(NullPointerException.class, () -> new OwoXmlTemplateRegistry(null, Map.of()));
        assertThrows(NullPointerException.class, () -> new OwoXmlTemplateRegistry(ignored -> null, null));
        assertThrows(IllegalArgumentException.class, () ->
            new OwoXmlTemplateRegistry(ignored -> null, java.util.Collections.singletonMap(" " , id)));
        assertThrows(NullPointerException.class, () ->
            new OwoXmlTemplateRegistry(ignored -> null, java.util.Collections.singletonMap("valid", null)));
    }
}
