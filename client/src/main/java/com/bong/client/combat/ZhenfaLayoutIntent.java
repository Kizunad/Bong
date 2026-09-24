package com.bong.client.combat;

import com.bong.client.ui.contract.UiIntent;

/** 阵法布置屏允许发送的语义动作；不暴露网络协议枚举。 */
public sealed interface ZhenfaLayoutIntent extends UiIntent permits ZhenfaLayoutIntent.Place {
    record Place(
        int x, int y, int z, String kind, String carrier, double qiInvestRatio,
        String trigger, Long itemInstanceId, String targetFace
    ) implements ZhenfaLayoutIntent {
        public Place {
            kind = required(kind, "kind");
            carrier = normalize(carrier);
            if (!Double.isFinite(qiInvestRatio) || qiInvestRatio < 0.0 || qiInvestRatio > 1.0) {
                throw new IllegalArgumentException("qi invest ratio must be finite within [0, 1]");
            }
            trigger = normalize(trigger);
            targetFace = normalize(targetFace);
            if (itemInstanceId != null && itemInstanceId < 0L) {
                throw new IllegalArgumentException("item instance id must be >= 0");
            }
        }

        private static String required(String value, String field) {
            String normalized = normalize(value);
            if (normalized == null) throw new IllegalArgumentException(field + " must not be blank");
            return normalized;
        }

        private static String normalize(String value) {
            return value == null || value.isBlank() ? null : value.strip();
        }
    }
}
