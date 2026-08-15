import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";

const projectRoot = process.cwd();
const targetRoot = path.join(projectRoot, "src-tauri", "target");
const targetTriple = process.env.FRP_PANEL_TARGET_TRIPLE ?? "unknown-target";
const outputPath = process.env.CHECKSUM_OUTPUT ?? path.join(projectRoot, "dist", `SHA256SUMS-${targetTriple}.txt`);
const supportedExtensions = new Set([".dmg", ".appimage", ".exe"]);

async function findReleaseArtifacts(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await findReleaseArtifacts(absolutePath)));
      continue;
    }
    if (!entry.isFile()) continue;
    const relativePath = path.relative(targetRoot, absolutePath);
    const extension = path.extname(entry.name).toLowerCase();
    if (relativePath.split(path.sep).includes("bundle") && supportedExtensions.has(extension)) {
      files.push(absolutePath);
    }
  }
  return files;
}

async function sha256(file) {
  const content = await readFile(file);
  return createHash("sha256").update(content).digest("hex");
}

try {
  const targetExists = await stat(targetRoot).then((entry) => entry.isDirectory());
  if (!targetExists) throw new Error(`Tauri target directory not found: ${targetRoot}`);
  const artifacts = (await findReleaseArtifacts(targetRoot)).sort();
  if (artifacts.length === 0) {
    throw new Error("No DMG, AppImage, or NSIS EXE release artifacts were found under src-tauri/target/**/bundle");
  }

  const lines = await Promise.all(
    artifacts.map(async (artifact) => {
      const hash = await sha256(artifact);
      return `${hash}  ${path.relative(targetRoot, artifact)}`;
    }),
  );
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${lines.join("\n")}\n`, "utf8");
  console.log(`Wrote ${lines.length} checksum(s) to ${outputPath}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
