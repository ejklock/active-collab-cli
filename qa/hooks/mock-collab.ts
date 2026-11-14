import { spawn } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const READY_TIMEOUT_MS = 20_000;
const READY_PREFIX = "mock-collab ready ";

const projectRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"..",
	"..",
);
const runRoot = path.join(projectRoot, "qa", ".run");
const nativeDebug = path.join(projectRoot, "target", "native", "debug");

/**
 * Starts the mock ActiveCollab on a fresh scratch store in the walkthrough's
 * own run folder `qa/.run/<name>/` (holding `ac.db`, `home/` and `tmp/`) and
 * resolves once it printed its ready line. Only that folder is reset, so
 * walkthroughs never share state. The mock exits when its stdin closes, so it
 * dies with the engine process that owns the pipe; the child and its pipes are
 * unref'd so the engine can exit on its own.
 */
export async function startMockCollab(name: string): Promise<void> {
	const runDir = path.join(runRoot, name);
	rmSync(runDir, { recursive: true, force: true });
	mkdirSync(path.join(runDir, "home"), { recursive: true });
	mkdirSync(path.join(runDir, "tmp"), { recursive: true });

	const child = spawn(
		path.join(nativeDebug, "examples", "mock_collab"),
		[path.join(runDir, "ac.db"), path.join(nativeDebug, "ac")],
		{ stdio: ["pipe", "pipe", "pipe"] },
	);

	await new Promise<void>((resolve, reject) => {
		let output = "";
		const timer = setTimeout(() => {
			child.kill();
			reject(
				new Error(
					`mock did not become ready in ${READY_TIMEOUT_MS} ms: ${output.trim()}`,
				),
			);
		}, READY_TIMEOUT_MS);

		const fail = (reason: string) => {
			clearTimeout(timer);
			reject(new Error(`${reason}: ${output.trim()}`));
		};

		child.stderr.on("data", (chunk: Buffer) => {
			output += chunk.toString();
		});
		child.stdout.on("data", (chunk: Buffer) => {
			output += chunk.toString();
			if (output.includes(READY_PREFIX)) {
				clearTimeout(timer);
				resolve();
			}
		});
		child.on("error", (error) => fail(`mock failed to start (${error.message})`));
		child.on("exit", (code) => fail(`mock exited with ${code} before it was ready`));
	});

	child.removeAllListeners("exit");
	child.removeAllListeners("error");
	child.stdout.removeAllListeners("data");
	child.stderr.removeAllListeners("data");
	child.stdout.unref();
	child.stderr.unref();
	child.stdin.unref();
	child.unref();
}
