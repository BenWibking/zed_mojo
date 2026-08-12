#!/usr/bin/env node

const assert = require("node:assert/strict");
const test = require("node:test");

const { rewriteMessage } = require("./mojo-lsp-zed.js");

test("preserves semantic-token delta support while removing notebook sync", () => {
  const response = {
    jsonrpc: "2.0",
    id: 1,
    result: {
      capabilities: {
        notebookDocumentSync: {
          notebookSelector: [],
        },
        semanticTokensProvider: {
          legend: {
            tokenTypes: ["type"],
            tokenModifiers: [],
          },
          full: {
            delta: true,
          },
        },
      },
    },
  };

  const rewritten = JSON.parse(rewriteMessage(JSON.stringify(response)));

  assert.equal(rewritten.result.capabilities.notebookDocumentSync, undefined);
  assert.deepEqual(rewritten.result.capabilities.semanticTokensProvider.full, {
    delta: true,
  });
});
