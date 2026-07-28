package com.bong.client.lifecycle;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
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
    void p0BaselineKeepsTheVerifiedThreeWayClassification() {
        assertEquals(
            106,
            ClientStoreScopeManifest.sessionScopedStores().size(),
            "P0 验真基线应有 106 个 session-scoped Store；数量变化时必须连同逐 FQCN source 对拍一起显式复核"
        );
        assertEquals(
            108,
            ClientStoreScopeManifest.allClassifiedStores().size(),
            "P0 验真基线应有 108 个业务 *Store.java（106 session + 1 persistent + 1 constant）"
        );
    }

    @Test
    void connectionStatusStoreRemainsTokenManagedOutsideTheGlobalRegistry() {
        assertEquals(
            Set.of("com.bong.client.ui.ClientConnectionStatusStore"),
            ClientStoreScopeManifest.externallyManagedSessionStores(),
            "连接状态 Store 必须由 handler token 精确失活，不能被无参全局 registry 清理"
        );
        assertFalse(
            ClientStoreScopeManifest.registryManagedSessionStores().contains(
                "com.bong.client.ui.ClientConnectionStatusStore"
            ),
            "ClientConnectionStatusStore 必须在 registry clear 之前由 invalidateSession(handler, now) 管理"
        );
        assertTrue(
            ClientStoreScopeManifest.sessionScopedStores().containsAll(
                ClientStoreScopeManifest.externallyManagedSessionStores()
            ),
            "externally managed 列表只能是 session-scoped Store 的子集"
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
    void registryExactlyMatchesManifestManagedSessionScope() {
        List<String> registeredFqcns = SessionScopedStoreRegistry.registeredFqcnsForTests();
        Set<String> registered = new HashSet<>(registeredFqcns);
        Set<String> expected = ClientStoreScopeManifest.registryManagedSessionStores();

        assertEquals(
            registered.size(),
            registeredFqcns.size(),
            "registry 不得重复登记同一 FQCN，否则断线会重复清理同一 Store"
        );
        assertEquals(
            expected,
            registered,
            "registry 必须与 manifest 的 registry-managed session Store 精确相等；漏登记="
                + difference(expected, registered) + "，越界登记=" + difference(registered, expected)
        );
        assertFalse(
            registered.contains("com.bong.client.ui.ClientConnectionStatusStore"),
            "ClientConnectionStatusStore 必须继续由 invalidateSession(handler, now) 管理，不能进入无参 registry"
        );
        assertFalse(
            registered.contains("com.bong.client.hud.HudLayoutPreferenceStore"),
            "HudLayoutPreferenceStore 是跨 session 的本地偏好，不能进入 registry"
        );
        assertFalse(
            registered.contains("com.bong.client.combat.ArmorProfileStore"),
            "ArmorProfileStore 是固定查表，不能进入 registry"
        );
    }

    @Test
    void registryUsesCanonicalProductionLifecycleMethodForEveryStore() throws IOException {
        String registrySource = Files.readString(
            ClientSourceTree.clientRoot().resolve(
                "src/main/java/com/bong/client/lifecycle/SessionScopedStoreRegistry.java"
            )
        );

        for (String fqcn : ClientStoreScopeManifest.registryManagedSessionStores()) {
            String storeSimpleName = fqcn.substring(fqcn.lastIndexOf('.') + 1);
            String canonicalCleaner = storeSimpleName + "::clearOnDisconnect";
            assertTrue(
                registrySource.contains(canonicalCleaner),
                fqcn + " 必须由 registry 绑定统一 production lifecycle 方法 clearOnDisconnect；"
                    + "不得退回 clear/clearAll/reset 或 test reset"
            );
            assertEquals(
                registrySource.indexOf(canonicalCleaner),
                registrySource.lastIndexOf(canonicalCleaner),
                fqcn + " 必须恰好登记一次 canonical lifecycle cleaner"
            );
        }
        assertFalse(
            registrySource.matches("(?s).*::(?:clear|clearAll|reset)\\).*"),
            "registry 不得再混用 clear/clearAll/reset；production disconnect 命名必须统一为 clearOnDisconnect"
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
