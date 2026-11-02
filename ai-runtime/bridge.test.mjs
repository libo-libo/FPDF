import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

process.env.FPDF_BRIDGE_TEST_IMPORT = '1';
const { parseConfig, selectModels, isDisabledModel } = await import('./bridge.mjs');
test('default model matches service aliases and Claude is excluded and rejected', () => {
  const ids = ['claude-sonnet-4', 'ANTHROPIC/anything', 'sonnet-4.5', 'Opus4', 'haiku-4.5',
    'GLM-5.3-Flash', 'gateway/DeepSeek-V4.1-Flash', 'gpt-5.4-mini'];
  const result = selectModels(ids.map(id => ({ id })));
  assert.equal(result.selected, 'gateway/DeepSeek-V4.1-Flash');
  assert.deepEqual(result.ids, ids.slice(5));
  for (const id of ids.slice(0,5)) {
    assert.ok(isDisabledModel(id));
    assert.throws(() => selectModels(ids.map(id => ({ id })), id), /已禁用/);
  }
  for (const alias of ['dsv4.1 flash', 'dsv4.1-flash', 'deepseek-v4-1-flash', 'provider/deepseek-v4.1-flash:high']) {
    assert.equal(selectModels([{ id: alias }]).selected, alias);
  }
  assert.equal(selectModels(ids.map(id => ({ id })), 'gpt-5.4-mini').selected, 'gpt-5.4-mini');
  assert.equal(selectModels([{ id: 'GLM-5.3-Flash' }]).selected, 'GLM-5.3-Flash');
  assert.throws(() => selectModels([{ id: 'claude-opus-4' }]), /已禁用/);
  const longCatalog = Array.from({ length: 110 }, (_, i) => ({ id: `model-${i}` }));
  longCatalog.push({ id: 'deepseek-v4.1-flash' });
  assert.equal(selectModels(longCatalog).selected, 'deepseek-v4.1-flash');
});

test('Anthropic-owned aliases are excluded even without a Claude name', () => {
  assert.deepEqual(selectModels([{ id: 'hidden-route', owned_by: 'anthropic' },
    { id: 'dsv4.1-flash' }]).ids, ['dsv4.1-flash']);
});

test('bundled bridge rejects a forbidden model before loading any service configuration', { timeout: 10000 }, async () => {
  const child = spawn(process.execPath, [resolve('../runtime/ai/bridge.mjs')], {
    env: { ...process.env, FPDF_BRIDGE_TEST_IMPORT: '' }, stdio: ['pipe', 'pipe', 'pipe'] });
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Forbidden model test timed out')), 5000);
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`bridge exited ${code}`)); });
      createInterface({ input: child.stdout }).on('line', line => {
        try {
          const event = JSON.parse(line);
          assert.equal(event.type, 'error');
          assert.match(event.text, /Claude.*已禁用/);
          clearTimeout(timer); resolve();
        } catch (error) { clearTimeout(timer); reject(error); }
      });
      child.stdin.write(JSON.stringify({ type: 'init', model: 'provider/Claude-Sonnet-4',
        config_path: 'deliberately-nonexistent-test-config.txt', document: { page_count: 0 } }) + '\n');
    });
  } finally { child.kill(); }
});

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
    res.end(JSON.stringify({ data: [{ id: 'claude-sonnet-4' }, { id: 'anthropic/opus-4' }, { id: 'deepseek-v4.1-flash' }] }));
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
        if (event.type === 'ready') { clearTimeout(timer); assert.equal(event.model, 'deepseek-v4.1-flash'); assert.deepEqual(event.models, ['deepseek-v4.1-flash']); resolve(); }
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
    if (req.url === '/v1/models') { res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ data: [{ id: 'gpt-5.4-mini' }, { id: 'deepseek-v4.1-flash' }] })); return; }
    let body = ''; for await (const data of req) body += data;
    const payload = JSON.parse(body);
    assert.equal(payload.model, 'deepseek-v4.1-flash');
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
        if (event.type === 'ready') { assert.equal(event.model, 'deepseek-v4.1-flash'); send({ type: 'prompt', page: 0, text: '总结当前页' }); }
        if (event.type === 'read_pages') { assert.deepEqual(event.pages, [0]); send({ type: 'pages', id: event.id, pages: [{ page: 0, text: 'Synthetic PDF evidence', truncated: false }] }); }
        if (event.type === 'delta') text += event.text;
        if (event.type === 'error') { clearTimeout(timer); reject(new Error(event.text)); }
        if (event.type === 'done') { clearTimeout(timer); try { assert.match(text, /第 1 页/); assert.equal(requests, 2); resolve(); } catch (e) { reject(e); } }
      });
      send({ type: 'init', config_path: join(dir, 'agent.txt'), document: { title: 'Synthetic test', page_count: 3, outline: [] } });
    });
  } finally { child.kill(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); }
});

test('full document survives large text, more than eight reads and follow-up context', { timeout: 30000 }, async () => {
  const dir = await mkdtemp(join(tmpdir(), 'fpdf-full-document-'));
  const count = 47;
  const evidence = page => `PAGE-${page}-BEGIN ` + '完整证据'.repeat(2500) + ` PAGE-${page}-END`;
  let requests = 0, completions = 0, batches = 0;
  const server = createServer(async (req, res) => {
    if (req.url === '/v1/models') {
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ data: [{ id: 'deepseek-v4.1-flash' }] })); return;
    }
    let body = ''; for await (const data of req) body += data;
    const payload = JSON.parse(body);
    const round = requests++;
    const tools = payload.messages.filter(m => m.role === 'tool');
    res.setHeader('Content-Type', 'text/event-stream');
    const emit = (delta, reason = null) => res.write(`data: ${JSON.stringify({ id: 'full-test', object: 'chat.completion.chunk', model: payload.model, choices: [{ index: 0, delta, finish_reason: reason }] })}\n\n`);
    // Defer assertions to the final checks; a failed server assertion must not hang a child.
    if (round < 9) {
      const pages = round === 0 ? Array.from({ length: 12 }, (_, i) => i + 1) : [round + 12];
      emit({ role: 'assistant', tool_calls: [{ index: 0, id: `read-${round}`, type: 'function', function: { name: 'read_pdf_pages', arguments: JSON.stringify({ pages }) } }] });
      emit({}, 'tool_calls');
    } else if (round === 9) {
      emit({ role: 'assistant', tool_calls: [{ index: 0, id: 'full-document', type: 'function', function: { name: 'read_pdf_document', arguments: '{}' } }] });
      emit({}, 'tool_calls');
    } else {
      const full = tools.find(m => String(m.content).includes(evidence(47)));
      const good = full && String(full.content).includes(evidence(1)) && String(full.content).includes(evidence(25))
        && payload.messages.some(m => m.role === 'user' && JSON.stringify(m.content).includes('QUESTION-END'));
      emit({ role: 'assistant', content: good ? 'FULL-EVIDENCE-OK [第 47 页]' : 'MISSING-EVIDENCE' });
      emit({}, 'stop');
    }
    res.end('data: [DONE]\n\n');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const config = join(dir, 'test-config.txt');
  await writeFile(config, `http://127.0.0.1:${server.address().port}/v1\nsynthetic-key\n`);
  const child = spawn(process.execPath, [resolve('../runtime/ai/bridge.mjs')], { env: { ...process.env, FPDF_BRIDGE_TEST_IMPORT: '' }, stdio: ['pipe', 'pipe', 'pipe'] });
  const send = obj => child.stdin.write(JSON.stringify(obj) + '\n');
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('full document timeout')), 25000);
      let answer = '';
      const fail = error => { clearTimeout(timer); reject(error); };
      child.once('exit', code => fail(new Error(`bridge exited ${code}`)));
      createInterface({ input: child.stdout }).on('line', line => {
        try {
          const event = JSON.parse(line);
          if (event.type === 'ready') send({ type: 'prompt', page: 0, text: '全文阅读 ' + 'Q'.repeat(13000) + ' QUESTION-END' });
          if (event.type === 'read_pages') {
            assert.ok(event.pages.length <= 5); batches++;
            send({ type: 'pages', id: event.id, pages: event.pages.map(page => ({ page, text: evidence(page + 1), truncated: false })) });
          }
          if (event.type === 'error') fail(new Error(event.text));
          if (event.type === 'delta') answer += event.text;
          if (event.type === 'done') {
            assert.match(answer, /FULL-EVIDENCE-OK/); answer = '';
            if (++completions === 1) send({ type: 'prompt', page: 0, text: '继续讨论上次全文证据' });
            else { assert.equal(requests, 12); assert.equal(batches, 21); clearTimeout(timer); resolve(); }
          }
        } catch (error) { fail(error); }
      });
      send({ type: 'init', config_path: config, document: { title: 'Synthetic full PDF', page_count: count, outline: [] } });
    });
  } finally { child.kill(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); }
});
async function exerciseAutomaticFallback(mode) {
  const dir = await mkdtemp(join(tmpdir(), 'fpdf-model-fallback-'));
  const attempts = [], switches = [], errors = [];
  let child, send, text = '', completions = 0;
  const catalog = ['claude-opus-4', 'deepseek-v4.1-flash', 'gpt-secondary', 'glm-backup'];
  const server = createServer(async (req, res) => {
    if (req.url === '/v1/models') {
      res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ data: catalog.map(id => ({ id })) })); return;
    }
    let body = ''; for await (const data of req) body += data;
    const payload = JSON.parse(body); attempts.push(payload.model);
    const emit = (delta, reason = null) => res.write(`data: ${JSON.stringify({ id: 'fallback', object: 'chat.completion.chunk', model: payload.model,
      choices: [{ index: 0, delta, finish_reason: reason }] })}\n\n`);
    if (mode === 'abort') {
      res.setHeader('Content-Type', 'text/event-stream'); emit({ role: 'assistant' });
      setTimeout(() => send({ type: 'abort' }), 10);
      return;
    }
    if (mode === 'partial') {
      res.setHeader('Content-Type', 'text/event-stream');
      emit({ role: 'assistant', content: 'Partial evidence kept.' }); emit({}, 'network_error');
      res.end('data: [DONE]\n\n'); return;
    }
    if (payload.model !== 'glm-backup' || mode === 'all-fail' || (mode === 'recover' && completions === 0)) {
      res.statusCode = payload.model === 'gpt-secondary' ? 429 : 403;
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ error: { message: 'model unavailable for this test', type: 'permission_error' } })); return;
    }
    res.setHeader('Content-Type', 'text/event-stream');
    emit({ role: 'assistant', content: 'Fallback answer OK.' }); emit({}, 'stop'); res.end('data: [DONE]\n\n');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const config = join(dir, 'test-config.txt');
  await writeFile(config, `http://127.0.0.1:${server.address().port}/v1\nfallback-synthetic-key\n`);
  child = spawn(process.execPath, [resolve('../runtime/ai/bridge.mjs')], { env: { ...process.env, FPDF_BRIDGE_TEST_IMPORT: '' }, stdio: ['pipe', 'pipe', 'pipe'] });
  send = obj => child.stdin.write(JSON.stringify(obj) + '\n');
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`fallback test timed out: ${mode}`)), 10000);
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`bridge exited ${code}`)); });
      createInterface({ input: child.stdout }).on('line', line => {
        try {
          const event = JSON.parse(line);
          if (event.type === 'ready') {
            assert.equal(event.model, 'deepseek-v4.1-flash');
            assert.ok(!event.models.some(isDisabledModel));
            send({ type: 'prompt', page: 0, text: 'Synthetic question one' });
          }
          if (event.type === 'model') switches.push(event.model);
          if (event.type === 'delta') text += event.text;
          if (event.type === 'error') errors.push(event.text);
          if (event.type === 'done') {
            completions++;
            if ((mode === 'success' || mode === 'recover') && completions === 1) send({ type: 'prompt', page: 0, text: 'Synthetic follow-up' });
            else { clearTimeout(timer); resolve(); }
          }
        } catch (error) { clearTimeout(timer); reject(error); }
      });
      send({ type: 'init', config_path: config, document: { title: 'Synthetic fallback test', page_count: 0, outline: [] } });
    });
    assert.ok(!attempts.some(isDisabledModel));
    if (mode === 'success') {
      assert.deepEqual(attempts, ['deepseek-v4.1-flash', 'gpt-secondary', 'glm-backup', 'glm-backup']);
      assert.deepEqual(switches, ['glm-backup']); assert.equal(errors.length, 0);
      assert.match(text, /Fallback answer OK/);
    } else if (mode === 'recover') {
      assert.deepEqual(attempts, ['deepseek-v4.1-flash', 'gpt-secondary', 'glm-backup', 'deepseek-v4.1-flash', 'gpt-secondary', 'glm-backup']);
      assert.deepEqual(switches, ['glm-backup']); assert.equal(errors.length, 1); assert.match(text, /Fallback answer OK/);
    } else if (mode === 'all-fail') {
      assert.deepEqual(attempts, ['deepseek-v4.1-flash', 'gpt-secondary', 'glm-backup']); assert.equal(errors.length, 1);
    } else {
      assert.deepEqual(attempts, ['deepseek-v4.1-flash']);
      if (mode === 'partial') { assert.match(text, /Partial evidence kept/); assert.equal(errors.length, 1); }
      assert.equal(switches.length, 0);
    }
  } finally { child.kill(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); }
}

for (const mode of ['success', 'all-fail', 'partial', 'abort', 'recover']) {
  test(`automatic model fallback: ${mode}`, { timeout: 15000 }, () => exerciseAutomaticFallback(mode));
}