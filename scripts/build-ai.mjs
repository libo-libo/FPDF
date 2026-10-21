// Run after npm ci in ai-runtime. Only runtime code and license texts are packaged.
import { readFile, writeFile, mkdir, readdir, copyFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
const root = resolve(import.meta.dirname, '..');
const project = join(root, 'ai-runtime');
const target = join(root, 'runtime/ai');
await mkdir(join(target, 'licenses'), { recursive: true });
execFileSync(process.execPath, [join(project, 'node_modules/esbuild/bin/esbuild'), 'bridge.mjs', '--bundle', '--platform=node', '--target=node24', '--format=esm', '--outfile=../runtime/ai/bridge.mjs'], { cwd: project, stdio: 'inherit' });
await copyFile(process.execPath, join(target, 'node.exe'));
const lock = JSON.parse(await readFile(join(project, 'package-lock.json'), 'utf8'));
const notices = ['# AI runtime dependencies', '', 'pi agent core and pi-ai: https://github.com/earendil-works/pi', 'Node.js: https://nodejs.org/', ''];
for (const [path, data] of Object.entries(lock.packages)) {
  if (!path || data.dev) continue;
  const name = path.replace(/^node_modules\//, '');
  const dir = join(project, path);
  const licenseFiles = (await readdir(dir)).filter(name => /^(license|licence|copying|notice)(\.|$)/i.test(name));
  for (const file of licenseFiles) {
    const destination = `${name.replaceAll('/', '__')}--${file}`;
    await copyFile(join(dir, file), join(target, 'licenses', destination));
  }
  notices.push(`- ${name} ${data.version}: ${data.license ?? 'see included license'}; ${data.resolved}`);
}
const version = process.version;
for (const [file, url] of [
  ['Node-LICENSE.txt', `https://raw.githubusercontent.com/nodejs/node/${version}/LICENSE`],
  ['pi-LICENSE.txt', 'https://raw.githubusercontent.com/earendil-works/pi/v1.3.0/LICENSE'],
]) {
  if (existsSync(join(target, 'licenses', file))) continue;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Cannot fetch ${file}`);
  await writeFile(join(target, 'licenses', file), await response.text());
}
await writeFile(join(target, 'THIRD-PARTY-NOTICES.md'), notices.join('\n') + '\n');
console.log('AI runtime and licenses prepared.');
