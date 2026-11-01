// Builds the Linux package in an Ubuntu 26.04 container (MehburOS' base), so it
// also works from Windows. Needs Docker. For `package`, the same signing key
// variables as `npm run package` must be set.
//   node scripts/package-linux.mjs test      Rust checks and tests
//   node scripts/package-linux.mjs package   signed .deb into target/linux
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";

const IMAGE = "mehburmc-linux-builder";
const VOLUME = "mehburmc-linux";
const mode = process.argv[2] ?? "package";
const out = resolve("target/linux");

const built = spawnSync("docker", ["build", "-q", "-t", IMAGE, "packaging/linux"], {
  stdio: "inherit",
});
if (built.status !== 0) process.exit(built.status ?? 1);
// Only the contents: Windows refuses to delete a folder a shell is standing in.
mkdirSync(out, { recursive: true });
for (const name of readdirSync(out)) rmSync(resolve(out, name), { recursive: true, force: true });

// Tracked and new files of the working tree; .github is the local-only folder.
const list = spawnSync("git", ["ls-files", "-co", "--exclude-standard", "-z", ":!.github"], {
  encoding: "buffer",
  maxBuffer: 1 << 28,
});
if (list.status !== 0) process.exit(1);

const tar = spawn("tar", ["-c", "-f", "-", "--null", "-T", "-"], {
  stdio: ["pipe", "pipe", "inherit"],
});
tar.stdin.end(list.stdout);

// RAM is tight on the machines this runs on: few jobs and a hard limit.
const docker = spawn(
  "docker",
  [
    "run", "--rm", "-i", "--memory=7g",
    "-e", "CARGO_BUILD_JOBS=3",
    "-e", "CARGO_TARGET_DIR=/work/target",
    "-e", "TAURI_SIGNING_PRIVATE_KEY",
    "-e", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    "-v", `${VOLUME}-src:/work/src`,
    "-v", `${VOLUME}-target:/work/target`,
    "-v", `${VOLUME}-cargo:/opt/cargo/registry`,
    "-v", `${out}:/out`,
    IMAGE, mode,
  ], // prettier-ignore
  { stdio: ["pipe", "inherit", "inherit"] },
);
tar.stdout.pipe(docker.stdin);
docker.on("exit", (code) => process.exit(code ?? 1));
