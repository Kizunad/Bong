package com.bong.client.ui.adapter.owo;

import com.bong.client.BongClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.util.Identifier;
import javax.imageio.ImageIO;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** 图片只在显式选择、刷新或资源重载时解码；先检查尺寸，再分配像素。 */
public final class WorkspaceBackgrounds {
    public record Entry(String id, String title) {}
    private final Path directory;
    private Identifier texture = builtin("cosmos");
    private int width = 1536;
    private int height = 1024;
    private boolean dynamic;
    private String selected = "cosmos";

    public WorkspaceBackgrounds(Path directory) { this.directory = directory; }
    public Path directory() { return directory; }
    public String selected() { return selected; }
    public List<Entry> entries() {
        List<Entry> entries = new ArrayList<>(List.of(new Entry("cosmos", "远空"), new Entry("terrain", "暮原")));
        try {
            Files.createDirectories(directory);
            try (var files = Files.list(directory)) {
                files.filter(Files::isRegularFile).filter(path -> {
                    String name = path.getFileName().toString().toLowerCase(java.util.Locale.ROOT);
                    return name.endsWith(".png") || name.endsWith(".jpg") || name.endsWith(".jpeg");
                }).sorted().limit(64).forEach(path -> entries.add(
                    new Entry("local:" + path.getFileName(), path.getFileName().toString())));
            }
        } catch (IOException failure) {
            BongClient.LOGGER.warn("无法读取工作台背景目录", failure);
        }
        return List.copyOf(entries);
    }

    public boolean select(String id) {
        if ("cosmos".equals(id) || "terrain".equals(id)) {
            release();
            texture = builtin(id);
            width = 1536;
            height = 1024;
            selected = id;
            return true;
        }
        if (id == null || !id.startsWith("local:")) return false;
        try {
            Path path = directory.resolve(id.substring(6)).normalize();
            if (!directory.normalize().equals(path.getParent())) return false;
            if (!Files.isRegularFile(path) || Files.size(path) > 16 * 1024 * 1024) return false;
            try (var source = ImageIO.createImageInputStream(path.toFile())) {
                if (source == null) return false;
                var readers = ImageIO.getImageReaders(source);
                if (!readers.hasNext()) return false;
                var reader = readers.next();
                NativeImage image;
                int w, h;
                try {
                    reader.setInput(source);
                    w = reader.getWidth(0);
                    h = reader.getHeight(0);
                    if (w < 1 || h < 1 || w > 4096 || h > 4096 || (long) w * h > 8_388_608) return false;
                    var decoded = reader.read(0);
                    image = new NativeImage(w, h, false);
                    for (int y = 0; y < h; y++) {
                        for (int x = 0; x < w; x++) {
                            int argb = decoded.getRGB(x, y);
                            image.setColor(x, y, (argb & 0xFF00FF00) | (argb >>> 16 & 255) | (argb & 255) << 16);
                        }
                    }
                } finally {
                    reader.dispose();
                }
                var replacement = new NativeImageBackedTexture(image);
                Identifier next;
                try {
                    next = MinecraftClient.getInstance().getTextureManager()
                        .registerDynamicTexture("bong-workspace", replacement);
                } catch (RuntimeException failure) {
                    replacement.close();
                    throw failure;
                }
                release();
                texture = next;
                dynamic = true;
                width = w;
                height = h;
                selected = id;
                return true;
            }
        } catch (IOException | RuntimeException failure) {
            BongClient.LOGGER.warn("无法加载本地工作台背景", failure);
            return false;
        }
    }

    public void render(DrawContext context, int viewportWidth, int viewportHeight) {
        drawCover(context, texture, width, height, 0, 0, viewportWidth, viewportHeight);
        context.fill(0, 0, viewportWidth, viewportHeight, 0x35080C0D);
    }

    public void reload() { select(selected); }

    public void thumbnail(DrawContext context, String id, int x, int y, int w, int h) {
        if (id.equals(selected)) drawCover(context, texture, width, height, x, y, w, h);
        else if (!id.startsWith("local:")) drawCover(context, builtin(id), 1536, 1024, x, y, w, h);
        else context.fill(x, y, x + w, y + h, 0xFF394540);
    }

    private static void drawCover(DrawContext context, Identifier texture, int tw, int th,
                                  int x, int y, int w, int h) {
        double scale = Math.max((double) w / tw, (double) h / th);
        int sourceWidth = Math.max(1, (int) Math.round(w / scale));
        int sourceHeight = Math.max(1, (int) Math.round(h / scale));
        context.drawTexture(texture, x, y, w, h, (tw - sourceWidth) / 2f, (th - sourceHeight) / 2f,
            sourceWidth, sourceHeight, tw, th);
    }

    private static Identifier builtin(String name) {
        return new Identifier("bong-client", "textures/gui/workspace/" + name + ".png");
    }

    private void release() {
        if (dynamic) MinecraftClient.getInstance().getTextureManager().destroyTexture(texture);
        dynamic = false;
    }
}
