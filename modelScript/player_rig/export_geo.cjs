// 复用 Bong 已有的 Blockbench Bedrock 离线 codec（client/tools/bbmodel_convert）导出几何。
// 用法：node export_geo.cjs <classic.bbmodel> <slim.bbmodel>，stdout 为合并后的 JSON。
const fs = require("node:fs");
const path = require("node:path");
const { exportModel } = require(path.join(__dirname, "../../client/tools/bbmodel_convert/blockbench_convert.js"));

const merged = { format_version: "1.12.0", "minecraft:geometry": [] };
for (const file of process.argv.slice(2)) {
    const geometry = JSON.parse(exportModel(JSON.parse(fs.readFileSync(file, "utf8"))));
    merged["minecraft:geometry"].push(...geometry["minecraft:geometry"]);
}
process.stdout.write(JSON.stringify(merged));
