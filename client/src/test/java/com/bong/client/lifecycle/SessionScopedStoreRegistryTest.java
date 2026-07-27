package com.bong.client.lifecycle;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SessionScopedStoreRegistryTest {
    @Test
    void clearsEveryHandleOnceInDeclarationOrder() {
        List<String> calls = new ArrayList<>();
        List<SessionStoreHandle> handles = List.of(
            handle("com.bong.test.FirstStore", () -> calls.add("first")),
            handle("com.bong.test.SecondStore", () -> calls.add("second")),
            handle("com.bong.test.ThirdStore", () -> calls.add("third"))
        );

        SessionScopedStoreRegistry.clearAllOnDisconnect(handles, failure -> {
            throw new AssertionError("正常清理不应产生 failure：" + failure.fqcn());
        });

        assertEquals(
            List.of("first", "second", "third"),
            calls,
            "registry 必须按显式声明顺序且每项恰好一次执行，避免断线清理顺序漂移或重复"
        );
    }

    @Test
    void runtimeFailureIsReportedAndDoesNotBlockLaterHandles() {
        List<String> calls = new ArrayList<>();
        RuntimeException expected = new IllegalStateException("broken store");
        List<SessionScopedStoreRegistry.StoreClearFailure> failures = new ArrayList<>();
        List<SessionStoreHandle> handles = List.of(
            handle("com.bong.test.FirstStore", () -> calls.add("first")),
            handle("com.bong.test.BrokenStore", () -> {
                calls.add("broken");
                throw expected;
            }),
            handle("com.bong.test.LastStore", () -> calls.add("last"))
        );

        SessionScopedStoreRegistry.clearAllOnDisconnect(handles, failures::add);

        assertEquals(
            List.of("first", "broken", "last"),
            calls,
            "单个 Store 的 RuntimeException 不得阻断后续断线清理"
        );
        assertEquals(1, failures.size(), "每个失败 Store 应报告一次且仅一次");
        assertEquals("com.bong.test.BrokenStore", failures.get(0).fqcn());
        assertEquals(expected, failures.get(0).cause());
    }

    @Test
    void errorIsNotSwallowed() {
        AssertionError expected = new AssertionError("fatal invariant");
        AssertionError actual = assertThrows(
            AssertionError.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                List.of(handle("com.bong.test.FatalStore", () -> {
                    throw expected;
                })),
                failure -> {
                    throw new AssertionError("Error 不应被转换成 RuntimeException failure");
                }
            )
        );

        assertEquals(expected, actual, "registry 只能隔离 RuntimeException，不能吞掉 Error");
    }

    @Test
    void rejectsDuplicateFqcnBeforeAnyClearRuns() {
        List<String> calls = new ArrayList<>();
        List<SessionStoreHandle> handles = List.of(
            handle("com.bong.test.DuplicateStore", () -> calls.add("first")),
            handle("com.bong.test.DuplicateStore", () -> calls.add("second"))
        );

        IllegalArgumentException exception = assertThrows(
            IllegalArgumentException.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(handles, failure -> {
            })
        );

        assertTrue(
            exception.getMessage().contains("com.bong.test.DuplicateStore"),
            "重复登记失败应指出具体 FQCN，实际=" + exception.getMessage()
        );
        assertTrue(calls.isEmpty(), "重复 key 必须在任何清理副作用前 fail-fast");
    }

    @Test
    void publicRegistryInspectionIsImmutable() {
        List<String> registered = SessionScopedStoreRegistry.registeredFqcnsForTests();
        assertThrows(
            UnsupportedOperationException.class,
            () -> registered.add("com.bong.test.IllegalMutationStore"),
            "registry inspection snapshot 必须不可变，测试不得动态篡改生产登记顺序"
        );
    }

    @Test
    void handleRejectsBlankFqcnAndNullClearer() {
        assertThrows(
            IllegalArgumentException.class,
            () -> new SessionStoreHandle(" ", () -> {
            }),
            "空白 FQCN 无法参与 source manifest 对拍，必须拒绝"
        );
        assertThrows(
            NullPointerException.class,
            () -> new SessionStoreHandle("com.bong.test.NullStore", null),
            "null clearer 会让断线时延迟 NPE，必须在构造时拒绝"
        );
    }

    private static SessionStoreHandle handle(String fqcn, SessionScopedStore clearer) {
        return new SessionStoreHandle(fqcn, clearer);
    }
}
