"use strict";

const { spawn } = require("node:child_process");

const child = spawn(
  "munin",
  ["hook", "session-start", "--format", "json"],
  {
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  },
);

let stdout = "";
let stderr = "";
child.stdout.on("data", (chunk) => {
  stdout += chunk.toString();
});
child.stderr.on("data", (chunk) => {
  stderr += chunk.toString();
});

const timer = setTimeout(() => {
  child.kill();
  process.stderr.write("Munin session-start receipt timed out.\n");
  process.exit(1);
}, 1500);

child.on("error", (error) => {
  clearTimeout(timer);
  process.stderr.write("Unable to start Munin: " + error.message + "\n");
  process.exit(1);
});

child.on("close", (code) => {
  clearTimeout(timer);
  if (code !== 0) {
    process.stderr.write(stderr || "Munin exited " + code + ".\n");
    process.exit(code || 1);
  }
  process.stdout.write(stdout);
});
