// Run the pinned shared controller tests and supply the app namespace to the
// integration tests in ui-runtime.test.mjs.
const { execFileSync } = require('node:child_process');
const { readFileSync } = require('node:fs');
const { dirname, join, resolve } = require('node:path');

const { packages } = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1'], {
  cwd: resolve(__dirname, '../../..'), encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
}));
const shell = packages.find(dep => dep.name === 'study-shell');
if (!shell) throw new Error('study-shell is missing from the workspace lockfile');
const root = dirname(shell.manifest_path);

exports.asset = name => readFileSync(join(root, 'src', name), 'utf8');
exports.source = () => exports.asset('system-back.js')
  .replaceAll('__study', '__kabar').replaceAll('studyBackTrap', 'kabarBackTrap');
if (require.main === module) require(join(root, 'tests/system-back.cjs'));
