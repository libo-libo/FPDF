import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

process.env.FPDF_BRIDGE_TEST_IMPORT = '1';
const { parseConfig } = await import('./bridge.mjs');
test('configuration rejects credentials in URL and accepts BOM/CRLF', () => {
  assert.deepEqual(parseConfig('\uFEFFhttp://localhost:9000/v1/\r\nlocal-test\r\n'), { baseUrl: 'http://localhost:9000/v1', apiKey: 'local-test' });
  assert.throws(() => parseConfig('http://user:secret@localhost/v1\nx'));
  assert.throws(() => parseConfig('file:///tmp/api\nx'));
  assert.throws(() => parseConfig('http://localhost/v1'));
});

test('connection can initialize models before any PDF is opened', { timeout: 20000 }, async () => {
  const dir = await mkdtemp(join(tmpdir(), 'fpdf-empty-document-'));
  let requests = 0;
  const server = createServer((req, res) => {
    assert.equal(req.headers.authorization, 'Bearer empty-document-test');
    assert.equal(req.url, '/v1/models');
    requests++;
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ data: [{ id: 'GLM-5.3-Flash' }] }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const config = join(dir, 'agent.txt');
  await writeFile(config, `http://127.0.0.1:${server.address().port}/v1\nempty-document-test\n`);
  const child = spawn(process.execPath, [resolve('../runtime/ai/bridge.mjs')], { env: { ...process.env, FPDF_BRIDGE_TEST_IMPORT: '' }, stdio: ['pipe', 'pipe', 'pipe'] });
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Empty-document connection timed out')), 15000);
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`bridge exited ${code}`)); });
      createInterface({ input: child.stdout }).on('line', line => {
        const event = JSON.parse(line);
        if (event.type === 'error') { clearTimeout(timer); reject(new Error(event.text)); }
        if (event.type === 'ready') { clearTimeout(timer); assert.equal(event.model, 'GLM-5.3-Flash'); resolve(); }
      });
      child.stdin.write(JSON.stringify({ type: 'init', config_path: config, document: { title: '', page_count: 0, outline: [] } }) + '\n');
    });
    assert.equal(requests, 1);
  } finally { child.kill(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); }
});

test('bundled pi agent requests PDF text, streams answer and uses authenticated API', { timeout: 30000 }, async () => {
  const dir = await mkdtemp(join(tmpdir(), 'fpdf-pi-'));
  let requests = 0;
  const server = createServer(async (req, res) => {
    assert.equal(req.headers.authorization, 'Bearer test-key');
    if (req.url === '/v1/models') { res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ data: [{ id: 'gpt-5.4-mini' }, { id: 'GLM-5.3-Flash' }] })); return; }
    let body = ''; for await (const data of req) body += data;
    const payload = JSON.parse(body);
    assert.equal(payload.model, 'GLM-5.3-Flash');
    assert.ok(payload.tools.some(t => t.function.name === 'read_pdf_pages'));
    assert.ok(!payload.tools.some(t => /shell|write|exec/i.test(t.function.name)));
    res.setHeader('Content-Type', 'text/event-stream');
    const emit = (delta, reason = null) => res.write(`data: ${JSON.stringify({ id: 'test', object: 'chat.completion.chunk', model: 'gpt-5.4-mini', choices: [{ index: 0, delta, finish_reason: reason }] })}\n\n`);
    if (++requests === 1) {
      emit({ role: 'assistant', tool_calls: [{ index: 0, id: 'read-1', type: 'function', function: { name: 'read_pdf_pages', arguments: '{"pages":[1]}' } }] });
      emit({}, 'tool_calls');
    } else {
      assert.ok(payload.messages.some(m => m.role === 'tool' && String(m.content).includes('Synthetic PDF evidence')));
      emit({ role: 'assistant', content: '已读取测试文档 [第 1 页]。' }); emit({}, 'stop');
    }
    res.end('data: [DONE]\n\n');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  await writeFile(join(dir, 'agent.txt'), `http://127.0.0.1:${server.address().port}/v1\ntest-key\n`);
  const child = spawn(process.execPath, [resolve('../runtime/ai/bridge.mjs')], { env: { ...process.env, FPDF_BRIDGE_TEST_IMPORT: '' }, stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = ''; child.stderr.on('data', b => stderr += b);
  const send = obj => child.stdin.write(JSON.stringify(obj) + '\n');
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('bridge timeout: ' + stderr)), 20000);
      let text = '';
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`bridge exited ${code}: ${stderr}`)); });
      createInterface({ input: child.stdout }).on('line', line => {
        const event = JSON.parse(line);
        if (event.type === 'ready') { assert.equal(event.model, 'GLM-5.3-Flash'); send({ type: 'prompt', page: 0, text: '总结当前页' }); }
        if (event.type === 'read_pages') { assert.deepEqual(event.pages, [0]); send({ type: 'pages', id: event.id, pages: [{ page: 0, text: 'Synthetic PDF evidence', truncated: false }] }); }
        if (event.type === 'delta') text += event.text;
        if (event.type === 'error') { clearTimeout(timer); reject(new Error(event.text)); }
        if (event.type === 'done') { clearTimeout(timer); try { assert.match(text, /第 1 页/); assert.equal(requests, 2); resolve(); } catch (e) { reject(e); } }
      });
      send({ type: 'init', config_path: join(dir, 'agent.txt'), document: { title: 'Synthetic test', page_count: 3, outline: [] } });
    });
  } finally { child.kill(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); }
});
