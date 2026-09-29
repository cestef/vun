// The VS Code client of Vun: starts the language server the manifest names under `server`.
const { LanguageClient } = require("vscode-languageclient/node");

let client;

exports.activate = (context) => {
  const manifest = context.extension.packageJSON;
  const language = manifest.contributes.languages[0].id;
  const { command, args } = manifest.server;
  client = new LanguageClient(language, manifest.displayName, { command, args }, {
    documentSelector: [{ scheme: "file", language }],
  });
  return client.start();
};

exports.deactivate = () => client?.stop();
