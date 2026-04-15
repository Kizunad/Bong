package com.bong.client.network;

/**
 * {@link VfxEventEnvelope#parse} 结果包裹。成功时携带 {@link VfxEventPayload}；失败时携带
 * 可直接写日志的错误信息。
 *
 * <p>与 {@link ServerPayloadParseResult} 保持一致的形态：总是非 null，调用方通过
 * {@link #isSuccess()} 分支处理。
 */
public final class VfxEventParseResult {
    private final boolean success;
    private final VfxEventPayload payload;
    private final String errorMessage;

    private VfxEventParseResult(boolean success, VfxEventPayload payload, String errorMessage) {
        this.success = success;
        this.payload = payload;
        this.errorMessage = errorMessage;
    }

    public static VfxEventParseResult success(VfxEventPayload payload) {
        return new VfxEventParseResult(true, payload, null);
    }

    public static VfxEventParseResult error(String errorMessage) {
        return new VfxEventParseResult(false, null, errorMessage);
    }

    public boolean isSuccess() {
        return success;
    }

    public VfxEventPayload payload() {
        return payload;
    }

    public String errorMessage() {
        return errorMessage;
    }
}
