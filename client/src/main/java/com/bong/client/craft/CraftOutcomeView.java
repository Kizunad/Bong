package com.bong.client.craft;

import java.util.Objects;

/** 手搓结果的 UI 投影，隔离 CraftStore 的事件实现类型。 */
public record CraftOutcomeView(
    Kind kind,
    String recipeId,
    String outputTemplate,
    int outputCount,
    long completedAtTick,
    String failureReason,
    int materialReturned,
    double qiRefunded
) {
    public CraftOutcomeView {
        Objects.requireNonNull(kind, "kind");
        Objects.requireNonNull(recipeId, "recipeId");
        outputTemplate = outputTemplate == null ? "" : outputTemplate;
        failureReason = failureReason == null ? "" : failureReason;
    }

    /** 只由状态适配层调用，UI 侧不需要依赖 Store 事件。 */
    static CraftOutcomeView from(CraftStore.CraftOutcomeEvent event) {
        Objects.requireNonNull(event, "event");
        return new CraftOutcomeView(
            event.kind() == CraftStore.CraftOutcomeEvent.Kind.COMPLETED
                ? Kind.COMPLETED : Kind.FAILED,
            event.recipeId(),
            event.outputTemplate(),
            event.outputCount(),
            event.completedAtTick(),
            event.failureReason(),
            event.materialReturned(),
            event.qiRefunded()
        );
    }

    public enum Kind {
        COMPLETED,
        FAILED
    }
}
