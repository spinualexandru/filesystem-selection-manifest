'use strict';

const { existsSync } = require('node:fs');
const { join } = require('node:path');

const localBuild = join(__dirname, 'index.node');

if (existsSync(localBuild)) {
  module.exports = require(localBuild);
} else {
  const packages = {
    'darwin-arm64': '@spinualexandru/fsmanifest-darwin-arm64',
    'darwin-x64': '@spinualexandru/fsmanifest-darwin-x64',
    'linux-arm64-gnu': '@spinualexandru/fsmanifest-linux-arm64-gnu',
    'linux-x64-gnu': '@spinualexandru/fsmanifest-linux-x64-gnu',
    'win32-x64-msvc': '@spinualexandru/fsmanifest-win32-x64-msvc',
  };
  const platform = currentPlatform();
  const packageName = packages[platform];

  if (packageName === undefined) {
    throw new Error(`fsmanifest does not provide a native binary for ${platform}`);
  }

  module.exports = require(packageName);
}

function currentPlatform() {
  if (process.platform === 'linux') {
    const report = process.report?.getReport();
    const header = report && typeof report === 'object' ? report.header : undefined;
    const abi = header && typeof header === 'object' && 'glibcVersionRuntime' in header
      ? 'gnu'
      : 'musl';
    return `linux-${process.arch}-${abi}`;
  }

  if (process.platform === 'win32') {
    return `win32-${process.arch}-msvc`;
  }

  return `${process.platform}-${process.arch}`;
}
