const state = {
  snapshot: null,
  health: null,
  activeTab: 'timeline',
};

const $ = (id) => document.getElementById(id);

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function formatPct(value) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) return 'n/a';
  return `${Math.round(Number(value) * 100)}%`;
}

function shortHash(value) {
  if (!value) return 'n/a';
  const text = String(value);
  if (text.length <= 22) return text;
  return `${text.slice(0, 14)}...${text.slice(-6)}`;
}

function addChat(role, html) {
  const article = document.createElement('article');
  article.className = `message ${role}`;
  article.innerHTML = `<div class="bubble">${html}</div>`;
  $('chatScroll').appendChild(article);
  $('chatScroll').scrollTop = $('chatScroll').scrollHeight;
}

async function apiGet(path) {
  const response = await fetch(path, { headers: { Accept: 'application/json' } });
  const body = await response.json();
  if (!response.ok || body.status === 'error') throw body;
  return body;
}

async function apiPost(path, payload) {
  const response = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(payload),
  });
  const body = await response.json();
  if (!response.ok || body.status === 'error') throw body;
  return body;
}

function renderHealth(value) {
  state.health = value;
  const el = $('health');
  const ollama = value.ollama || {};
  el.className = `health ${value.status === 'ok' && ollama.status === 'ok' ? 'ok' : 'blocked'}`;
  el.textContent = value.status === 'ok' && ollama.status === 'ok' ? 'ready' : 'blocked';
}

function metricCard(label, value, meaning) {
  return `<article class="metric"><div class="label">${escapeHtml(label)}</div><div class="value">${escapeHtml(value)}</div><div class="meaning">${escapeHtml(meaning)}</div></article>`;
}

function renderMetricGrid(snapshot) {
  if (!snapshot) {
    $('metricGrid').innerHTML = metricCard('Source of truth', 'waiting', 'Load a real run or start a live Gemma run.');
    return;
  }
  const metrics = snapshot.metrics || {};
  const verification = metrics.verification || {};
  const score = metrics.score || {};
  const training = metrics.training_episode_yield || {};
  const surprise = metrics.surprise || {};
  const reward = metrics.reward || {};
  $('metricGrid').innerHTML = [
    metricCard('SOT coverage', formatPct(verification.source_of_truth_coverage), 'How much of the action was checked by durable readback.'),
    metricCard('Readback ratio', formatPct(verification.readback_ratio), 'Verifier rows that include actual source-of-truth evidence.'),
    metricCard('Trust', score.trust_level || 'n/a', 'Run-level trust state computed from verifier/evidence rows.'),
    metricCard('Incidents', snapshot.ledger?.run_counts?.incident_packet ?? 0, 'Intentional bad readback creates a real incident packet.'),
    metricCard('Training episodes', training.dynamicjepa_episode_count ?? 'n/a', 'Action-conditioned examples exported for evaluation/training.'),
    metricCard('Surprise', surprise.surprise_events ?? 'n/a', 'Unexpected outcome records produced by DynamicJEPA.'),
    metricCard('Reward', reward.raw ?? 'not compiled', 'Verified outcome reward compiled from scorecard features.'),
    metricCard('Loss', reward.loss ?? 'not compiled', 'Training loss form of the reward signal.'),
  ].join('');
}

function renderTimeline(snapshot) {
  if (!snapshot) {
    $('timeline').innerHTML = '<article class="timeline-item"><div class="badge missing">waiting</div><div class="timeline-body"><h3>No run loaded</h3><p>Start a live run or inspect a completed real run.</p></div></article>';
    return;
  }
  $('timeline').innerHTML = (snapshot.timeline || []).map((item) => `
    <article class="timeline-item">
      <div><span class="badge ${escapeHtml(item.status)}">${escapeHtml(item.stage)}</span></div>
      <div class="timeline-body">
        <h3>${escapeHtml(item.title)}</h3>
        <p><strong>${escapeHtml(item.value)}</strong></p>
        <p>${escapeHtml(item.meaning)}</p>
        <p><code>${escapeHtml(JSON.stringify(item.evidence || {}, null, 2))}</code></p>
      </div>
    </article>
  `).join('');
}

function renderMetricsDetail(snapshot) {
  if (!snapshot) {
    $('metricsDetail').innerHTML = '';
    return;
  }
  const groups = snapshot.metrics || {};
  $('metricsDetail').innerHTML = Object.entries(groups).map(([name, value]) => `
    <article class="detail-card">
      <h3>${escapeHtml(name)}</h3>
      <pre>${escapeHtml(JSON.stringify(value, null, 2))}</pre>
    </article>
  `).join('');
}

function renderLedger(snapshot) {
  if (!snapshot) {
    $('ledgerDetail').innerHTML = '';
    return;
  }
  const ledger = snapshot.ledger || {};
  const rows = ledger.latest_rows || [];
  $('ledgerDetail').innerHTML = `
    <div class="detail-grid">
      <article class="detail-card">
        <h3>Physical Ledger Readback</h3>
        <pre>${escapeHtml(JSON.stringify({
          source_of_truth: ledger.source_of_truth,
          record_count: ledger.record_count,
          tip: ledger.tip,
          physical_counts: ledger.physical_counts,
          run_counts: ledger.run_counts,
        }, null, 2))}</pre>
      </article>
      <article class="detail-card">
        <h3>Run Identity</h3>
        <pre>${escapeHtml(JSON.stringify(snapshot.identity, null, 2))}</pre>
      </article>
    </div>
    <div class="table-wrap" style="margin-top: 10px;">
      <table>
        <thead><tr><th>Sequence</th><th>Kind</th><th>Run</th><th>Record</th><th>Hash</th></tr></thead>
        <tbody>${rows.map((row) => `
          <tr>
            <td>${escapeHtml(row.sequence)}</td>
            <td>${escapeHtml(row.record_kind)}</td>
            <td><code>${escapeHtml(shortHash(row.run_id))}</code></td>
            <td><code>${escapeHtml(shortHash(row.record_id))}</code></td>
            <td><code>${escapeHtml(shortHash(row.record_sha256))}</code></td>
          </tr>
        `).join('')}</tbody>
      </table>
    </div>
  `;
}

function renderCorrelations(snapshot) {
  if (!snapshot) {
    $('correlations').innerHTML = '';
    return;
  }
  const gaps = snapshot.current_gaps || [];
  $('correlations').innerHTML = `
    <div class="correlations-list">
      ${(snapshot.correlations || []).map((item) => `
        <article class="correlation-card">
          <h3>${escapeHtml(item.name)} <span class="badge ${item.status === 'linked' ? 'passed' : 'missing'}">${escapeHtml(item.status)}</span></h3>
          <p>${escapeHtml(item.from)} -> ${escapeHtml(item.to)}</p>
          <p>${escapeHtml(item.meaning)}</p>
          <p><code>${escapeHtml((item.join_keys || []).join(', '))}</code></p>
        </article>
      `).join('')}
      ${gaps.map((gap) => `
        <article class="correlation-card">
          <h3>Gap: ${escapeHtml(gap.gap || gap.name || 'unnamed')}</h3>
          <p>${escapeHtml(gap.why_it_matters || '')}</p>
          <p>${escapeHtml(gap.recommended_fix || '')}</p>
        </article>
      `).join('')}
    </div>
  `;
}

function renderArtifacts(snapshot) {
  if (!snapshot) {
    $('artifacts').innerHTML = '';
    return;
  }
  $('artifacts').innerHTML = `
    <div class="artifacts-list">
      ${(snapshot.artifacts || []).map((item) => `
        <article class="artifact-row">
          <h3>${escapeHtml(item.name)}</h3>
          <p><code>${escapeHtml(item.path)}</code></p>
          <p>bytes=${escapeHtml(item.bytes)} sha=${escapeHtml(shortHash(item.sha256))}</p>
          <p>${escapeHtml(item.source_of_truth)}</p>
        </article>
      `).join('')}
    </div>
  `;
}

function renderRaw(snapshot) {
  $('rawJson').textContent = snapshot ? JSON.stringify(snapshot, null, 2) : '';
}

function renderSnapshot(snapshot) {
  state.snapshot = snapshot;
  renderMetricGrid(snapshot);
  renderTimeline(snapshot);
  renderMetricsDetail(snapshot);
  renderLedger(snapshot);
  renderCorrelations(snapshot);
  renderArtifacts(snapshot);
  renderRaw(snapshot);
}

function appendEvent(event) {
  const el = document.createElement('div');
  el.className = `event ${event.level === 'error' ? 'error' : ''}`;
  el.innerHTML = `<strong>${escapeHtml(event.stage)}</strong> ${escapeHtml(event.message)}<br><code>${escapeHtml(JSON.stringify(event.details || {}))}</code>`;
  $('events').prepend(el);
  while ($('events').children.length > 80) $('events').lastChild.remove();
}

async function refreshHealth() {
  try {
    const value = await apiGet('/api/health');
    renderHealth(value);
  } catch (err) {
    renderHealth({ status: 'error', ollama: err });
  }
}

async function loadLatestRun() {
  addChat('user', '<p>Load the latest completed real run and show the source-of-truth readback.</p>');
  try {
    const runs = await apiGet('/api/runs');
    const latest = (runs.runs || []).find((item) => item.complete);
    if (!latest) throw { error_code: 'NO_REAL_RUN', message: 'No completed real run exists under /tmp.' };
    const snapshot = await apiPost('/api/inspect', { root: latest.root, compile_reward: false });
    renderSnapshot(snapshot);
    addChat('assistant', `<p>Loaded <code>${escapeHtml(latest.root)}</code>. The right pane is reading physical files and SQLite ledger state from that run.</p>`);
  } catch (err) {
    addChat('assistant', `<p><strong>${escapeHtml(err.error_code || 'ERROR')}</strong>: ${escapeHtml(err.message || err)}</p>`);
  }
}

async function runLive(event) {
  event.preventDefault();
  const prompt = $('prompt').value.trim();
  addChat('user', `<p>${escapeHtml(prompt || '(empty prompt)')}</p>`);
  try {
    const snapshot = await apiPost('/api/chat', { message: prompt, compile_reward: true });
    renderSnapshot(snapshot);
    addChat('assistant', `<p>Gemma requested the governed tool and ContextGraph produced evidence. BlackBoxFix run: <code>${escapeHtml(snapshot.identity.blackbox_run_id)}</code>.</p>`);
  } catch (err) {
    addChat('assistant', `<p><strong>${escapeHtml(err.error_code || 'ERROR')}</strong>: ${escapeHtml(err.message || err)}</p><p>${escapeHtml(err.remediation || '')}</p>`);
  }
}

function setupTabs() {
  document.querySelectorAll('.tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      document.querySelectorAll('.tab').forEach((item) => item.classList.remove('active'));
      document.querySelectorAll('.tab-panel').forEach((item) => item.classList.remove('active'));
      tab.classList.add('active');
      $(`panel-${tab.dataset.tab}`).classList.add('active');
    });
  });
}

function setupEvents() {
  const source = new EventSource('/api/events');
  source.addEventListener('system', (event) => {
    appendEvent(JSON.parse(event.data));
  });
  source.onerror = () => {
    appendEvent({ stage: 'events', level: 'error', message: 'Event stream disconnected', details: {} });
  };
}

document.addEventListener('DOMContentLoaded', () => {
  setupTabs();
  setupEvents();
  refreshHealth();
  renderSnapshot(null);
  $('refreshHealth').addEventListener('click', refreshHealth);
  $('loadLatest').addEventListener('click', loadLatestRun);
  $('chatForm').addEventListener('submit', runLive);
  document.querySelectorAll('.prompt-chip').forEach((button) => {
    button.addEventListener('click', () => {
      $('prompt').value = button.dataset.prompt || '';
      $('prompt').focus();
    });
  });
});
