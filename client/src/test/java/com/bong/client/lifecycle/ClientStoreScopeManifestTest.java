package com.bong.client.lifecycle;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientStoreScopeManifestTest {
    private static final String LIFECYCLE_INTERFACE =
        "com.bong.client.lifecycle.SessionScopedStore";

    @Test
    void everyProductionStoreHasExactlyOneExplicitScope() throws IOException {
        Set<String> discovered = discoverProductionStores();
        Set<String> session = ClientStoreScopeManifest.sessionScopedStores();
        Set<String> persistent = ClientStoreScopeManifest.persistentConfigStores();
        Set<String> constant = ClientStoreScopeManifest.constantStores();

        assertDisjoint(session, persistent, "session-scoped", "persistent-config");
        assertDisjoint(session, constant, "session-scoped", "constant");
        assertDisjoint(persistent, constant, "persistent-config", "constant");

        assertEquals(
            discovered,
            new TreeSet<>(ClientStoreScopeManifest.allClassifiedStores()),
            "每个 production *Store.java 必须恰好落入 manifest 的一种 scope；新增 Store 时必须显式判定"
                + " session-scoped / persistent-config / constant，且 lifecycle interface 本身不能污染业务发现集"
        );
    }

    @Test
    void persistentPreferenceAndConstantLookupStayOutOfSessionScope() {
        assertEquals(
            Set.of("com.bong.client.hud.HudLayoutPreferenceStore"),
            ClientStoreScopeManifest.persistentConfigStores(),
            "HUD 布局是本地用户偏好，断线必须保留，不能误归 session-scoped"
        );
        assertEquals(
            Set.of("com.bong.client.combat.ArmorProfileStore"),
            ClientStoreScopeManifest.constantStores(),
            "护甲 profile 是固定查表，断线必须保留，不能误归 session-scoped"
        );
        assertFalse(
            ClientStoreScopeManifest.sessionScopedStores().contains(
                "com.bong.client.hud.HudLayoutPreferenceStore"
            ),
            "persistent-config Store 不得同时进入 session-scoped"
        );
        assertFalse(
            ClientStoreScopeManifest.sessionScopedStores().contains(
                "com.bong.client.combat.ArmorProfileStore"
            ),
            "constant Store 不得同时进入 session-scoped"
        );
    }

    @Test
    void p0RegistryIsAnExplicitSessionScopeSubset() {
        Set<String> registered = new HashSet<>(SessionScopedStoreRegistry.registeredFqcnsForTests());
        assertEquals(
            registered.size(),
            SessionScopedStoreRegistry.registeredFqcnsForTests().size(),
            "registry 不得重复登记同一 FQCN，否则断线会重复清理同一 Store"
        );
        assertTrue(
            ClientStoreScopeManifest.sessionScopedStores().containsAll(registered),
            "P0 registry 只允许登记 manifest 已分类为 session-scoped 的 Store；P3 再收紧为全集相等，实际越界="
                + difference(registered, ClientStoreScopeManifest.sessionScopedStores())
        );
    }

    private static Set<String> discoverProductionStores() throws IOException {
        Path javaRoot = ClientSourceTree.clientRoot().resolve("src/main/java");
        Path clientPackage = javaRoot.resolve("com/bong/client");
        TreeSet<String> discovered = new TreeSet<>();
        try (Stream<Path> paths = Files.walk(clientPackage)) {
            paths.filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith("Store.java"))
                .map(path -> toFqcn(javaRoot.relativize(path)))
                .filter(fqcn -> !LIFECYCLE_INTERFACE.equals(fqcn))
                .forEach(discovered::add);
        }
        return discovered;
    }

    private static String toFqcn(Path relativeJavaPath) {
        int nameCount = relativeJavaPath.getNameCount();
        String fileName = relativeJavaPath.getName(nameCount - 1).toString();
        String simpleName = fileName.substring(0, fileName.length() - ".java".length());
        StringBuilder fqcn = new StringBuilder();
        for (int index = 0; index < nameCount - 1; index++) {
            if (index > 0) {
                fqcn.append('.');
            }
            fqcn.append(relativeJavaPath.getName(index));
        }
        return fqcn.append('.').append(simpleName).toString();
    }

    private static void assertDisjoint(
        Set<String> left,
        Set<String> right,
        String leftLabel,
        String rightLabel
    ) {
        Set<String> overlap = new TreeSet<>(left);
        overlap.retainAll(right);
        assertTrue(
            overlap.isEmpty(),
            "Store scope 必须互斥；" + leftLabel + " 与 " + rightLabel + " 重叠=" + overlap
        );
    }

    private static Set<String> difference(Set<String> left, Set<String> right) {
        Set<String> difference = new TreeSet<>(left);
        difference.removeAll(right);
        return difference;
    }
}
