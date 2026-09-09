import com.bong.client.hud.svg.NanoSvgParser;
import com.bong.client.hud.svg.SvgDocument;
import com.bong.client.hud.svg.SvgMesh;
import com.bong.client.hud.svg.SvgTessellator;

import java.awt.Color;
import java.awt.RenderingHints;
import java.awt.geom.Path2D;
import java.awt.image.BufferedImage;
import java.nio.file.Files;
import java.nio.file.Path;
import javax.imageio.ImageIO;

/** 预览工具：直接绘制生产 tessellator 的 mesh，输出不等于 Minecraft 截图。 */
public class RenderAssets {
    public static void main(String[] args) throws Exception {
        Path root = Path.of(args[0]);
        Path output = Path.of(args[1]);
        var report = new StringBuilder("asset,shapes,triangles,bounds\n");
        String[] folders = args.length > 2
            ? java.util.Arrays.copyOfRange(args, 2, args.length)
            : new String[]{"round-1", "round-2"};
        for (String folder : folders) {
            Path destination = output.resolve(folder);
            Files.createDirectories(destination);
            try (var files = Files.list(root.resolve(folder))) {
                for (Path file : files.filter(p -> p.toString().endsWith(".svg")).sorted().toList()) {
                    SvgDocument document;
                    try (var input = Files.newInputStream(file)) {
                        document = new NanoSvgParser().parse(input);
                    }
                    SvgMesh mesh = new SvgTessellator().tessellate(document);
                    if (mesh.triangleCount() == 0) throw new IllegalStateException("空 mesh: " + file);
                    int scale = 4;
                    var picture = new BufferedImage((int) document.width() * scale,
                        (int) document.height() * scale, BufferedImage.TYPE_INT_ARGB);
                    var g = picture.createGraphics();
                    g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
                    // 同一 SVG 图元的三角形一次填充，避免 Java2D 在内部边缘重复抗锯齿。
                    for (var shape : document.shapes()) {
                        SvgMesh shapeMesh = new SvgTessellator().tessellate(
                            new SvgDocument(document.width(), document.height(), java.util.List.of(shape)));
                        if (shapeMesh.triangleCount() == 0) continue;
                        var polygon = new Path2D.Float();
                        for (var triangle : shapeMesh.triangles()) {
                            boolean first = true;
                            for (var vertex : new SvgMesh.Vertex[]{triangle.a(), triangle.b(), triangle.c()}) {
                                if (vertex.x() < 0 || vertex.y() < 0 || vertex.x() > document.width()
                                    || vertex.y() > document.height()) {
                                    throw new IllegalStateException("越界 mesh: " + file);
                                }
                                if (first) polygon.moveTo(vertex.x() * scale, vertex.y() * scale);
                                else polygon.lineTo(vertex.x() * scale, vertex.y() * scale);
                                first = false;
                            }
                            polygon.closePath();
                        }
                        g.setColor(new Color(shapeMesh.triangles().get(0).a().color(), true));
                        g.fill(polygon);
                    }
                    g.dispose();
                    ImageIO.write(picture, "png", destination.resolve(file.getFileName().toString().replace(".svg", ".png")).toFile());
                    report.append(folder).append('/').append(file.getFileName()).append(',')
                        .append(document.shapes().size()).append(',').append(mesh.triangleCount()).append(",OK\n");
                }
            }
        }
        // 注入不受支持的滤镜，证明此处确实执行生产白名单门禁。
        try {
            new NanoSvgParser().parse(new java.io.ByteArrayInputStream(
                "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><filter/></svg>".getBytes(java.nio.charset.StandardCharsets.UTF_8)));
            throw new AssertionError("未拒绝注入的滤镜");
        } catch (IllegalArgumentException expected) {
            report.append("injected-filter,REJECTED\n");
        }
        Files.writeString(output.resolve("asset-check.csv"), report);
        System.out.print(report);
    }
}
