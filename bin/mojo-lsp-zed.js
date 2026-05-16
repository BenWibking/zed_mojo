#!/usr/bin/env node

const { spawn } = require("node:child_process");

const serverPath = process.argv[2] || "mojo-lsp-server";
const serverArgs = process.argv.slice(3);

const child = spawn(serverPath, serverArgs, {
  stdio: ["pipe", "pipe", "pipe"],
  env: {
    ...process.env,
    MODULAR_TELEMETRY_ENABLED: process.env.MODULAR_TELEMETRY_ENABLED ?? "0",
  },
});

process.stdin.pipe(child.stdin);
child.stderr.pipe(process.stderr);

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 0);
  }
});

let buffer = Buffer.alloc(0);

child.stdout.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  flushMessages();
});

function flushMessages() {
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) {
      return;
    }

    const header = buffer.subarray(0, headerEnd).toString("ascii");
    const match = /^Content-Length:\s*(\d+)/im.exec(header);
    if (!match) {
      process.stdout.write(buffer);
      buffer = Buffer.alloc(0);
      return;
    }

    const bodyLength = Number(match[1]);
    const messageStart = headerEnd + 4;
    const messageEnd = messageStart + bodyLength;
    if (buffer.length < messageEnd) {
      return;
    }

    const body = buffer.subarray(messageStart, messageEnd).toString("utf8");
    buffer = buffer.subarray(messageEnd);
    writeMessage(rewriteMessage(body));
  }
}

function rewriteMessage(body) {
  let message;
  try {
    message = JSON.parse(body);
  } catch {
    return body;
  }

  const capabilities = message?.result?.capabilities;
  if (capabilities && typeof capabilities === "object") {
    delete capabilities.notebookDocumentSync;

    const semantic = capabilities.semanticTokensProvider;
    if (semantic && typeof semantic === "object" && semantic.full && typeof semantic.full === "object") {
      semantic.full = true;
    }
  }

  return JSON.stringify(message);
}

function writeMessage(body) {
  process.stdout.write(`Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n${body}`);
}
