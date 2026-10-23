// Run after npm ci in ai-runtime. Only runtime code and license texts are packaged.
import { readFile, writeFile, mkdir, readdir, copyFile } from 'node:fs/promises';
import { resolve, join, dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { existsSync } from 'node:fs';
const root = resolve(import.meta.dirname, '..');
const project = join(root, 'ai-runtime');
const target = join(root, 'runtime/ai');
const { build } = await import(pathToFileURL(join(project, 'node_modules/esbuild/lib/main.js')).href);
await mkdir(join(target, 'licenses'), { recursive: true });
await build({ entryPoints: [join(project, 'bridge.mjs')], bundle: true, platform: 'node', target: 'node24', format: 'esm', outfile: join(target, 'bridge.mjs'), logLevel: 'info' });
const nodeName = process.platform === 'win32' ? 'node.exe' : 'node';
await copyFile(process.env.FPDF_RUNTIME_NODE || process.execPath, join(target, nodeName));
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
const runtimeNode = process.env.FPDF_RUNTIME_NODE || process.execPath;
const nodeRoot = dirname(runtimeNode);
const nodeLicense = [join(nodeRoot, 'LICENSE'), join(nodeRoot, '..', 'LICENSE')].find(file => existsSync(file));
if (!nodeLicense) throw new Error('Node.js distribution LICENSE was not found beside the runtime.');
await copyFile(nodeLicense, join(target, 'licenses', 'Node-LICENSE.txt'));
const piLicense = join(root, 'third-party/licenses/pi-1.3.0/LICENSE');
if (!existsSync(piLicense)) throw new Error(`Missing pi license: ${piLicense}`);
await copyFile(piLicense, join(target, 'licenses', 'pi-LICENSE.txt'));
await writeFile(join(target, 'THIRD-PARTY-NOTICES.md'), notices.join('\n') + '\n');
console.log('AI runtime and licenses prepared.');
