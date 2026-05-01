package com.bong.client.visual.realm_vision;

public final class RealmVisionFogController {
    private static final FogParamsSink GL_SINK = new GlFogParamsSink();
    private static volatile FogParamsSink sink = GL_SINK;

    private RealmVisionFogController() {
    }

    public static void apply(long tick) {
        RealmVisionCommand command = RealmVisionPlanner.plan(RealmVisionStateStore.snapshot(), tick);
        if (command != null) {
            sink.apply(command);
        }
    }

    public static void setSinkForTests(FogParamsSink testSink) {
        sink = testSink == null ? GL_SINK : testSink;
    }
}
