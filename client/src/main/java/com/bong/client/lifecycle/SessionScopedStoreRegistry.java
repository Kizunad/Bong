package com.bong.client.lifecycle;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.Consumer;

public final class SessionScopedStoreRegistry {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/session-store-lifecycle");
    private static final List<SessionStoreHandle> REGISTERED = List.of();

    private SessionScopedStoreRegistry() {
    }

    public static void clearAllOnDisconnect() {
        clearAllOnDisconnect(
            REGISTERED,
            failure -> LOGGER.error(
                "Failed to clear session store {} on disconnect",
                failure.fqcn(),
                failure.cause()
            )
        );
    }

    static void clearAllOnDisconnect(
        List<SessionStoreHandle> handles,
        Consumer<StoreClearFailure> failureHandler
    ) {
        Objects.requireNonNull(handles, "handles");
        Objects.requireNonNull(failureHandler, "failureHandler");
        validateUniqueFqcns(handles);
        List<StoreClearFailure> failures = new ArrayList<>();
        for (SessionStoreHandle handle : handles) {
            try {
                handle.clearOnDisconnect();
            } catch (RuntimeException exception) {
                failures.add(new StoreClearFailure(handle.fqcn(), exception));
            }
        }
        RuntimeException reportingFailure = null;
        for (StoreClearFailure failure : failures) {
            try {
                failureHandler.accept(failure);
            } catch (RuntimeException exception) {
                if (reportingFailure == null) {
                    reportingFailure = exception;
                } else {
                    reportingFailure.addSuppressed(exception);
                }
            }
        }
        if (reportingFailure != null) {
            throw reportingFailure;
        }
    }

    static List<String> registeredFqcnsForTests() {
        return REGISTERED.stream().map(SessionStoreHandle::fqcn).toList();
    }

    static void validateUniqueFqcns(List<SessionStoreHandle> handles) {
        Set<String> seen = new HashSet<>();
        List<String> duplicates = new ArrayList<>();
        for (SessionStoreHandle handle : handles) {
            Objects.requireNonNull(handle, "handle");
            if (!seen.add(handle.fqcn())) {
                duplicates.add(handle.fqcn());
            }
        }
        if (!duplicates.isEmpty()) {
            throw new IllegalArgumentException("Duplicate session store FQCNs: " + duplicates);
        }
    }

    record StoreClearFailure(String fqcn, RuntimeException cause) {
        StoreClearFailure {
            Objects.requireNonNull(fqcn, "fqcn");
            Objects.requireNonNull(cause, "cause");
        }
    }
}
