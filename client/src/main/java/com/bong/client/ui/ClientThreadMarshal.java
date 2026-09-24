package com.bong.client.ui;

import net.minecraft.client.MinecraftClient;

import java.util.Objects;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * R7 P4 的客户端线程边界。
 *
 * <p>这个小适配器只负责判断并派发任务，不拥有网络接收语义、Screen 状态或待处理队列。
 * R6 的网络接收边界继续由 {@code BongNetworkHandler} 独占。</p>
 */
public final class ClientThreadMarshal {
    private ClientThreadMarshal() {
    }

    /** 在真实 Minecraft client 上执行一次；client 尚未创建时 fail closed。 */
    public static boolean run(Runnable task) {
        Objects.requireNonNull(task, "task must not be null");
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return false;
        }
        return run(client::isOnThread, task, client::execute);
    }

    /**
     * 可测试的线程派发 seam。
     *
     * <p>谓词为 {@code true} 时内联执行；为 {@code false} 时只入队一次；谓词返回 null
     * 表示线程状态不可知，任务既不执行也不入队。任务或 executor 的异常保持原样传播。</p>
     */
    static boolean run(
        Supplier<Boolean> onClientThread,
        Runnable task,
        Consumer<Runnable> clientExecutor
    ) {
        Objects.requireNonNull(onClientThread, "onClientThread must not be null");
        Objects.requireNonNull(task, "task must not be null");
        Objects.requireNonNull(clientExecutor, "clientExecutor must not be null");

        Boolean onThread = onClientThread.get();
        if (onThread == null) {
            return false;
        }
        if (onThread) {
            task.run();
        } else {
            clientExecutor.accept(task);
        }
        return true;
    }
}
