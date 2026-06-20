package com.bong.client.movement;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class MovementKeybindingsTest {

    private static final double EPS = 1e-9;

    // ---- 参考帧：玩家体朝向 yaw（无 WASD 输入时即朝正前 dash）----

    @Test
    void noInputDashesAlongPlayerYaw() {
        assertEquals(
            20.0,
            MovementKeybindings.resolveDashYawDegrees(20.0, 0.0, 0.0),
            EPS,
            "无方向键时 dash yaw 应等于玩家 yaw(20)，即朝体朝向正前"
        );
    }

    @Test
    void dashYawIsCameraIndependentSoFrontAndBackViewMatch() {
        // 第一/三人称背视/三人称正面视唯一差别是相机 yaw；参考帧只用玩家 yaw，
        // 故同一玩家 yaw + 同一 WASD 在三种视角下结果恒等——这正是「正面视 == 背面视」。
        double playerYaw = 30.0;
        double back = MovementKeybindings.resolveDashYawDegrees(playerYaw, -1.0, 1.0);
        double front = MovementKeybindings.resolveDashYawDegrees(playerYaw, -1.0, 1.0);
        assertEquals(back, front, EPS,
            "dash 朝向不依赖相机，三种视角下同输入应给出同一 yaw");
        assertEquals(playerYaw - 135.0, back, EPS,
            "玩家 yaw 30 + A+S 偏移(-135) = -105");
    }

    // ---- WASD 偏移角：覆盖 8 向 + 无输入。基准方向角 = atan2(-sideways, forward) ----
    // 约定：forward W=+1/S=-1，sideways A=+1(左)/D=-1(右)。

    @Test
    void noDirectionalInputYieldsZeroOffset() {
        assertEquals(0.0, MovementKeybindings.wasdYawOffsetDegrees(0.0, 0.0), EPS,
            "无方向键 → 0 偏移，保持朝视线正前 dash");
    }

    @Test
    void forwardKeyYieldsZeroOffset() {
        assertEquals(0.0, MovementKeybindings.wasdYawOffsetDegrees(1.0, 0.0), EPS,
            "W 应为正前(0°)");
    }

    @Test
    void backKeyYields180() {
        assertEquals(180.0, Math.abs(MovementKeybindings.wasdYawOffsetDegrees(-1.0, 0.0)), EPS,
            "S 应为正后(±180°)");
    }

    @Test
    void leftKeyYieldsMinus90() {
        assertEquals(-90.0, MovementKeybindings.wasdYawOffsetDegrees(0.0, 1.0), EPS,
            "A(左, sideways=+1) 应为 -90°");
    }

    @Test
    void rightKeyYieldsPlus90() {
        assertEquals(90.0, MovementKeybindings.wasdYawOffsetDegrees(0.0, -1.0), EPS,
            "D(右, sideways=-1) 应为 +90°");
    }

    @Test
    void forwardLeftYieldsMinus45() {
        assertEquals(-45.0, MovementKeybindings.wasdYawOffsetDegrees(1.0, 1.0), EPS,
            "W+A 应为前偏左 -45°");
    }

    @Test
    void forwardRightYieldsPlus45() {
        assertEquals(45.0, MovementKeybindings.wasdYawOffsetDegrees(1.0, -1.0), EPS,
            "W+D 应为前偏右 +45°");
    }

    @Test
    void backLeftYieldsMinus135() {
        // 用户点名场景：A+S → 在当前视角下「后偏左 45°」的后退 dash。
        assertEquals(-135.0, MovementKeybindings.wasdYawOffsetDegrees(-1.0, 1.0), EPS,
            "A+S(后偏左) 应为 -135°——后退轴再往左旋 45°");
    }

    @Test
    void backRightYieldsPlus135() {
        assertEquals(135.0, MovementKeybindings.wasdYawOffsetDegrees(-1.0, -1.0), EPS,
            "D+S(后偏右) 应为 +135°");
    }

    @Test
    void offsetIsScaleInvariant() {
        // 潜行/缓行等会等比缩放 forward/sideways，定向只看符号与比例，角度不变。
        assertEquals(
            MovementKeybindings.wasdYawOffsetDegrees(1.0, 1.0),
            MovementKeybindings.wasdYawOffsetDegrees(0.3, 0.3),
            EPS,
            "等比缩放（潜行 0.3×）不应改变 dash 朝向角"
        );
    }

    // ---- 合成：玩家 yaw + WASD 偏移 一起作用 ----

    @Test
    void combinedYawAddsOffsetOntoPlayerYaw() {
        // 玩家朝 yaw=90，按 A+S → 90 + (-135) = -45。
        assertEquals(
            -45.0,
            MovementKeybindings.resolveDashYawDegrees(90.0, -1.0, 1.0),
            EPS,
            "合成 yaw 应为 玩家 yaw(90) + A+S 偏移(-135) = -45"
        );
    }

    @Test
    void combinedYawRightStrafeAddsNinety() {
        // 玩家 yaw=10，按 D → 10 + 90 = 100。
        assertEquals(
            100.0,
            MovementKeybindings.resolveDashYawDegrees(10.0, 0.0, -1.0),
            EPS,
            "玩家 yaw(10) 上叠加 D 偏移(+90) = 100"
        );
    }
}
