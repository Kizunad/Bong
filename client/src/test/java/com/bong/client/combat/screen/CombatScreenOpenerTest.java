package com.bong.client.combat.screen;

import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.ui.ScreenTransition;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.ScreenTransitionRegistry;
import net.minecraft.client.gui.screen.Screen;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CombatScreenOpenerTest {
    @AfterEach
    void clearStores() {
        DeathStateStore.hide();
        TerminateStateStore.hide();
    }

    @Test
    void terminationReplacesDeathAndClosesWithoutRestartingTransitions() {
        DeferredScreenHost host = new DeferredScreenHost();
        DeathStateStore.replace(new DeathStateStore.State(true, "test", 0, List.of(), 0, false, false));
        host.poll();
        assertInstanceOf(DeathScreen.class, host.current);

        TerminateStateStore.replace(new TerminateStateStore.State(true, "", "", ""));
        DeathStateStore.hide();
        host.poll();
        var opening = host.transition.handle();
        host.pollFrames();
        host.finishAtDeadline(opening);
        assertInstanceOf(TerminateScreen.class, host.current,
            "每帧轮询不能取消最初的转场，终结屏必须在原定时间打开");
        assertNull(host.transition, "开屏完成后必须释放输入锁");

        TerminateStateStore.hide();
        host.poll();
        var closing = host.transition.handle();
        host.pollFrames();
        host.finishAtDeadline(closing);
        assertNull(host.current, "服务端隐藏终结屏后，反复轮询不能重启关屏动画");
        assertNull(host.transition, "关屏完成后必须恢复游戏输入");
    }

    @Test
    void hidingTerminationBeforeOpenCancelsItsPendingDelivery() {
        DeferredScreenHost host = new DeferredScreenHost();
        TerminateStateStore.replace(new TerminateStateStore.State(true, "", "", ""));
        host.poll();
        var opening = host.transition.handle();
        TerminateStateStore.hide();
        host.pollFrames();
        host.finishAtDeadline(opening);
        assertNull(host.current, "服务端已经隐藏的终结屏不得在旧动画回调中重新出现");
        assertNull(host.transition, "取消尚未打开的终结屏不得残留输入锁");
    }

    /** 模拟 setScreen 的延迟交付，使用生产转场配置和取消／完成语义，无需 OpenGL。 */
    private static final class DeferredScreenHost {
        private Screen current;
        private ScreenTransitionController.ActiveTransition transition;

        void poll() {
            CombatScreenOpener.tick(current, transition, this::setScreen);
        }

        void pollFrames() {
            for (int frame = 0; frame < 20; frame++) {
                poll();
            }
        }

        void setScreen(Screen next) {
            if (transition != null) {
                transition.handle().cancel();
                transition = null;
            }
            if (current == next) {
                return;
            }
            var spec = ScreenTransitionRegistry.resolve(current, next);
            var handle = ScreenTransition.play(current, next, spec.type(), spec.durationMs(), spec.easing(), () -> {
                current = next;
                transition = null;
            });
            if (!handle.completed()) {
                transition = new ScreenTransitionController.ActiveTransition(handle, spec, handle.startedAtMs());
            }
        }

        void finishAtDeadline(ScreenTransition.TransitionHandle handle) {
            var frame = handle.sample(handle.startedAtMs() + handle.durationMs(), 320, 180);
            assertTrue(frame.finished());
            assertFalse(frame.inputLocked());
            handle.complete();
        }
    }
}
