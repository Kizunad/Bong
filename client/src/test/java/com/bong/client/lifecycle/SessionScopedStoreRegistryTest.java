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
            handle(FirstStore.class, () -> calls.add("first")),
            handle(SecondStore.class, () -> calls.add("second")),
            handle(ThirdStore.class, () -> calls.add("third"))
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
            handle(FirstStore.class, () -> calls.add("first")),
            handle(BrokenStore.class, () -> {
                calls.add("broken");
                throw expected;
            }),
            handle(LastStore.class, () -> calls.add("last"))
        );

        SessionScopedStoreRegistry.clearAllOnDisconnect(handles, failures::add);

        assertEquals(
            List.of("first", "broken", "last"),
            calls,
            "单个 Store 的 RuntimeException 不得阻断后续断线清理"
        );
        assertEquals(1, failures.size(), "每个失败 Store 应报告一次且仅一次");
        assertEquals(BrokenStore.class.getName(), failures.get(0).fqcn());
        assertEquals(expected, failures.get(0).cause());
    }

    @Test
    void reportingFailureCannotBlockRemainingStoreClears() {
        List<String> calls = new ArrayList<>();
        RuntimeException reportingFailure = new IllegalStateException("reporter broken");
        List<SessionStoreHandle> handles = List.of(
            handle(FirstStore.class, () -> calls.add("first")),
            handle(BrokenStore.class, () -> {
                calls.add("broken");
                throw new IllegalStateException("store broken");
            }),
            handle(LastStore.class, () -> calls.add("last"))
        );

        RuntimeException actual = assertThrows(
            RuntimeException.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                handles,
                failure -> {
                    calls.add("report:" + failure.fqcn());
                    throw reportingFailure;
                }
            )
        );

        assertEquals(reportingFailure, actual, "失败上报异常应在全部 Store 清理完成后原样透传");
        assertEquals(
            List.of("first", "broken", "last", "report:" + BrokenStore.class.getName()),
            calls,
            "失败上报必须与 Store 清理遍历解耦，reporter 异常不得跳过后续 Store"
        );
    }

    @Test
    void everyReportingFailureIsAttemptedAfterAllStoreClears() {
        List<String> calls = new ArrayList<>();
        RuntimeException firstReportingFailure = new IllegalStateException("first reporter broken");
        RuntimeException secondReportingFailure = new IllegalArgumentException("second reporter broken");
        List<SessionStoreHandle> handles = List.of(
            handle(FirstStore.class, () -> {
                calls.add("first");
                throw new IllegalStateException("first store broken");
            }),
            handle(SecondStore.class, () -> calls.add("second")),
            handle(BrokenStore.class, () -> {
                calls.add("broken");
                throw new IllegalStateException("second store broken");
            }),
            handle(LastStore.class, () -> calls.add("last"))
        );

        RuntimeException actual = assertThrows(
            RuntimeException.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                handles,
                failure -> {
                    calls.add("report:" + failure.fqcn());
                    if (failure.fqcn().equals(FirstStore.class.getName())) {
                        throw firstReportingFailure;
                    }
                    throw secondReportingFailure;
                }
            )
        );

        assertEquals(
            firstReportingFailure,
            actual,
            "首个失败上报异常应在所有 Store 清理和上报尝试结束后原样透传"
        );
        assertEquals(
            List.of(secondReportingFailure),
            List.of(actual.getSuppressed()),
            "后续失败上报异常必须保留为 suppressed，不能让诊断信息静默丢失"
        );
        assertEquals(
            List.of(
                "first",
                "second",
                "broken",
                "last",
                "report:" + FirstStore.class.getName(),
                "report:" + BrokenStore.class.getName()
            ),
            calls,
            "所有 Store 必须先完成清理，随后每个 Store failure 都必须获得一次上报尝试"
        );
    }

    @Test
    void repeatedReportingFailureInstanceDoesNotAbortRemainingReports() {
        List<String> calls = new ArrayList<>();
        RuntimeException reportingFailure = new IllegalStateException("shared reporter broken");
        List<SessionStoreHandle> handles = List.of(
            handle(FirstStore.class, () -> {
                calls.add("first");
                throw new IllegalStateException("first store broken");
            }),
            handle(BrokenStore.class, () -> {
                calls.add("broken");
                throw new IllegalArgumentException("second store broken");
            }),
            handle(LastStore.class, () -> {
                calls.add("last");
                throw new IllegalStateException("third store broken");
            })
        );

        RuntimeException actual = assertThrows(
            RuntimeException.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                handles,
                failure -> {
                    calls.add("report:" + failure.fqcn());
                    throw reportingFailure;
                }
            )
        );

        assertEquals(
            reportingFailure,
            actual,
            "同一 reporter 异常实例重复抛出时必须原样透传，不能因 self-suppression 改抛 IllegalArgumentException"
        );
        assertEquals(
            List.of(),
            List.of(actual.getSuppressed()),
            "同一异常实例不能 suppressed 自身，也不应制造重复诊断条目"
        );
        assertEquals(
            List.of(
                "first",
                "broken",
                "last",
                "report:" + FirstStore.class.getName(),
                "report:" + BrokenStore.class.getName(),
                "report:" + LastStore.class.getName()
            ),
            calls,
            "共享 reporter 异常实例不得阻断后续 failure 的上报尝试"
        );
    }

    @Test
    void reportingErrorIsNotSwallowedAfterStoreClearsComplete() {
        List<String> calls = new ArrayList<>();
        AssertionError expected = new AssertionError("fatal reporter invariant");
        List<SessionStoreHandle> handles = List.of(
            handle(FirstStore.class, () -> {
                calls.add("first");
                throw new IllegalStateException("store broken");
            }),
            handle(LastStore.class, () -> calls.add("last"))
        );

        AssertionError actual = assertThrows(
            AssertionError.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                handles,
                failure -> {
                    calls.add("report:" + failure.fqcn());
                    throw expected;
                }
            )
        );

        assertEquals(expected, actual, "registry 不能吞掉或转换 failure reporter 的 Error");
        assertEquals(
            List.of("first", "last", "report:" + FirstStore.class.getName()),
            calls,
            "failure reporter 的 Error 只能在所有 Store 完成清理后透传"
        );
    }

    @Test
    void errorIsNotSwallowed() {
        AssertionError expected = new AssertionError("fatal invariant");
        AssertionError actual = assertThrows(
            AssertionError.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(
                List.of(handle(FatalStore.class, () -> {
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
            handle(DuplicateStore.class, () -> calls.add("first")),
            handle(DuplicateStore.class, () -> calls.add("second"))
        );

        IllegalArgumentException exception = assertThrows(
            IllegalArgumentException.class,
            () -> SessionScopedStoreRegistry.clearAllOnDisconnect(handles, failure -> {
            })
        );

        assertTrue(
            exception.getMessage().contains(DuplicateStore.class.getName()),
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
    void handleDerivesFqcnFromStoreTypeAndRejectsNullInputs() {
        SessionStoreHandle handle = handle(FirstStore.class, () -> {
        });

        assertEquals(
            FirstStore.class.getName(),
            handle.fqcn(),
            "handle 身份必须从 Store class 派生，调用方不得手填可与 clearer 脱节的 FQCN"
        );
        assertEquals(FirstStore.class, handle.storeType());
        assertThrows(
            NullPointerException.class,
            () -> SessionStoreHandle.forStore(null, () -> {
            }),
            "null Store class 无法派生 manifest 身份，必须在构造时拒绝"
        );
        assertThrows(
            NullPointerException.class,
            () -> SessionStoreHandle.forStore(FirstStore.class, null),
            "null clearer 会让断线时延迟 NPE，必须在构造时拒绝"
        );
    }

    private static SessionStoreHandle handle(Class<?> storeType, SessionScopedStore clearer) {
        return SessionStoreHandle.forStore(storeType, clearer);
    }

    private static final class FirstStore {
    }

    private static final class SecondStore {
    }

    private static final class ThirdStore {
    }

    private static final class BrokenStore {
    }

    private static final class LastStore {
    }

    private static final class FatalStore {
    }

    private static final class DuplicateStore {
    }
}
