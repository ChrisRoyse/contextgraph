const state = {
  snapshot: null,
  health: null,
  activeTab: 'timeline',
  clientId: `client-${crypto.randomUUID()}`,
  activeRequestId: null,
  latestProductPack: null,
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

function renderAgentTruth(snapshot) {
  const agent = snapshot.chat_agent || {};
  if (agent.selected_tool) {
    const selected = agent.selected_tool || {};
    const result = agent.tool_result_summary || {};
    const final = agent.final_model_response || {};
    return `
      <p><strong>What Gemma actually did</strong></p>
      <p>Gemma selected <code>${escapeHtml(selected.name)}</code> from the ContextGraph tool surface. ContextGraph validated the arguments, executed the tool, read durable state, then sent the verified tool result back to Gemma for the explanation below.</p>
      <div class="truth-grid">
        <div><span>Operator prompt hash</span><code>${escapeHtml(shortHash(agent.operator_prompt_sha256))}</code></div>
        <div><span>Tool call id</span><code>${escapeHtml(selected.tool_call_id || 'n/a')}</code></div>
        <div><span>Selected tool</span><code>${escapeHtml(selected.name)}</code></div>
        <div><span>Evidence source</span><code>${escapeHtml(shortHash(result.source_of_truth || agent.source_of_truth))}</code></div>
      </div>
      <details class="truth-details">
        <summary>Show the exact arguments Gemma requested</summary>
        <pre>${escapeHtml(JSON.stringify(selected.arguments || {}, null, 2))}</pre>
      </details>
      <details class="truth-details">
        <summary>Show verified tool result summary</summary>
        <pre>${escapeHtml(JSON.stringify(result, null, 2))}</pre>
      </details>
      <p class="truth-note"><strong>Deterministic evidence summary:</strong> ${escapeHtml(result.verified_summary_text || 'n/a')}</p>
      <p class="gemma-final">${escapeHtml(final.content || final.content_redacted_preview || '')}</p>
    `;
  }
  const interaction = snapshot.model_interaction || {};
  const contract = interaction.tool_contract || {};
  const modelMessage = interaction.actual_model_message || {};
  const toolCalls = Array.isArray(modelMessage.tool_calls) ? modelMessage.tool_calls : [];
  const firstCall = toolCalls[0] || {};
  const fn = firstCall.function || {};
  const args = typeof fn.arguments === 'string' ? fn.arguments : JSON.stringify(fn.arguments || {}, null, 2);
  return `
    <p><strong>What Gemma actually did</strong></p>
    <p>Gemma produced a native tool call. ContextGraph executed the tool only after validating the allowlisted name, scenario, and required capabilities.</p>
    <div class="truth-grid">
      <div><span>Operator prompt hash</span><code>${escapeHtml(shortHash(interaction.operator_prompt_sha256))}</code></div>
      <div><span>Tool call id</span><code>${escapeHtml(firstCall.id || 'n/a')}</code></div>
      <div><span>Tool name</span><code>${escapeHtml(fn.name || 'n/a')}</code></div>
      <div><span>Execution boundary</span><code>${escapeHtml(contract.mode || 'n/a')}</code></div>
    </div>
    <details class="truth-details">
      <summary>Show the exact tool arguments Gemma requested</summary>
      <pre>${escapeHtml(args)}</pre>
    </details>
    <p class="truth-note">${escapeHtml(contract.executor || 'ContextGraph executes the tool; Gemma only requests it.')}</p>
  `;
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

function newRequestId(kind) {
  return `${kind}-${crypto.randomUUID()}`;
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
    metricCard('Loss', reward.loss ?? 'not compiled', 'Minimization form of reward; this compiler uses loss = -reward.'),
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

function renderToolSurface(snapshot) {
  const agent = snapshot?.chat_agent;
  if (!agent) {
    $('toolSurface').innerHTML = '<article class="tool-surface-card"><h3>Tool Surface</h3><p>Ask Gemma a ContextGraph question to see which tool it chooses.</p></article>';
    return;
  }
  const selected = agent.selected_tool || {};
  const tools = agent.available_tool_names || [];
  $('toolSurface').innerHTML = `
    <article class="tool-surface-card">
      <h3>Gemma Tool Choice</h3>
      <p><strong>${escapeHtml(selected.name || 'n/a')}</strong></p>
      <p>${escapeHtml(agent.tool_contract?.executor || '')}</p>
      <div class="tool-pill-row">${tools.map((name) => `<span class="tool-pill ${name === selected.name ? 'selected' : ''}">${escapeHtml(name)}</span>`).join('')}</div>
    </article>
  `;
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

function renderProductCategory(snapshot) {
  const fullPack = snapshot?.product_category_pack;
  const compactPack = snapshot?.chat_agent?.tool_result_summary?.product_category_pack;
  const pack = fullPack || compactPack || state.latestProductPack;
  if (!pack) {
    $('productCategory').innerHTML = `
      <article class="detail-card">
        <h3>Product Category Pack</h3>
        <p>Ask Gemma to build the investor product category pack. ContextGraph will aggregate real completed runs, write category artifacts, sign the manifest, and prove tamper detection.</p>
      </article>
    `;
    return;
  }
  const executive = pack.executive_view || {};
  const arena = pack.model_arena || {};
  const arenaRows = arena.rows || [];
  const leaderboard = (pack.verified_outcome_leaderboard?.rows || pack.leaderboard_top || []);
  const failureCases = pack.failure_injection_studio?.failure_cases || pack.failure_case_status || [];
  const reward = pack.reward_dashboard || {};
  const rewardRows = reward.reward_history || [];
  const attestation = pack.attestation || {};
  const signed = attestation.signed_attestation || {};
  const tamper = attestation.tamper_test || {};
  const connector = pack.connector_contracts || {};
  const governance = pack.governance_evidence_map || {};
  const artifacts = pack.artifacts || {};
  $('productCategory').innerHTML = `
    <div class="product-hero">
      <div>
        <p class="eyebrow">Product category</p>
        <h3>${escapeHtml(executive.product_category || 'Verified Outcome Intelligence for Tool-Using AI Agents')}</h3>
        <p>${escapeHtml(executive.positioning || 'ContextGraph is the verification, benchmark, evidence, and reward layer for tool-using AI agents.')}</p>
      </div>
      <div class="product-pack-id">
        <span>Pack</span>
        <code>${escapeHtml(shortHash(pack.pack_id))}</code>
        <span>Source</span>
        <code>${escapeHtml(shortHash(pack.pack_file?.source_of_truth || pack.source_of_truth))}</code>
      </div>
    </div>

    <div class="product-metrics">
      ${metricCard('Actions', executive.actions_attempted ?? 'n/a', 'Tool calls measured from real runs.')}
      ${metricCard('Evidence', executive.evidence_packages ?? 'n/a', 'Persisted evidence packets counted from ledgers.')}
      ${metricCard('Incidents', executive.incident_caught ?? 'n/a', 'Failures caught by verifier/readback evidence.')}
      ${metricCard('Training', executive.training_episodes_generated ?? 'n/a', 'Action-conditioned episodes materialized.')}
    </div>

    <article class="detail-card">
      <h3>Model Comparison Arena</h3>
      <p class="detail-copy">${escapeHtml(arena.honesty_note || 'Only measured model runs are shown.')}</p>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Model</th><th>Runs</th><th>Tool accuracy</th><th>Verifier pass</th><th>Incident rate</th><th>Reward</th><th>Risk</th></tr></thead>
          <tbody>${arenaRows.map((row) => `
            <tr>
              <td>${escapeHtml(row.model)}</td>
              <td>${escapeHtml(row.runs_total)}</td>
              <td>${escapeHtml(formatPct(row.tool_choice_accuracy))}</td>
              <td>${escapeHtml(formatPct(row.verifier_pass_rate))}</td>
              <td>${escapeHtml(formatPct(row.incident_rate))}</td>
              <td>${escapeHtml(row.average_reward ?? 'n/a')}</td>
              <td>${escapeHtml(row.average_risk ?? 'n/a')}</td>
            </tr>
          `).join('')}</tbody>
        </table>
      </div>
    </article>

    <article class="detail-card">
      <h3>Verified Outcome Leaderboard</h3>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Rank</th><th>Run</th><th>Trust</th><th>SOT</th><th>Incidents</th><th>Reward</th><th>Confidence</th></tr></thead>
          <tbody>${leaderboard.slice(0, 8).map((row) => `
            <tr>
              <td>${escapeHtml(row.rank ?? '')}</td>
              <td><code>${escapeHtml(shortHash(row.blackbox_run_id))}</code></td>
              <td>${escapeHtml(row.trust_level)}</td>
              <td>${escapeHtml(formatPct(row.source_of_truth_coverage))}</td>
              <td>${escapeHtml(row.incident_count)}</td>
              <td>${escapeHtml(row.reward ?? 'n/a')}</td>
              <td>${escapeHtml(row.verification_confidence ?? 'n/a')}</td>
            </tr>
          `).join('')}</tbody>
        </table>
      </div>
    </article>

    <div class="product-grid">
      <article class="detail-card">
        <h3>Failure Injection Studio</h3>
        <div class="failure-list">${failureCases.map((item) => `
          <div><span class="badge ${item.status === 'caught' || item.status === 'passed' || item.status === 'tamper_detected' ? 'passed' : 'blocked'}">${escapeHtml(item.status)}</span><strong>${escapeHtml(item.id)}</strong><p>${escapeHtml(item.meaning || '')}</p></div>
        `).join('')}</div>
      </article>
      <article class="detail-card">
        <h3>Signed Attestation</h3>
        <pre>${escapeHtml(JSON.stringify({
          status: signed.status || attestation.status,
          manifest: signed.manifest?.source_of_truth,
          signature: signed.signature?.sha256,
          tamper_status: tamper.status || attestation.tamper_test_status,
          tamper_exit_code: tamper.verify_command?.exit_code ?? attestation.tamper_exit_code,
        }, null, 2))}</pre>
      </article>
    </div>

    <div class="product-grid">
      <article class="detail-card">
        <h3>Reward Compiler Dashboard</h3>
        <pre>${escapeHtml(JSON.stringify({
          loss_convention: reward.loss_convention || 'loss = -reward',
          latest_reward: reward.latest_reward || pack.reward_latest,
          history_count: rewardRows.length || undefined,
        }, null, 2))}</pre>
      </article>
      <article class="detail-card">
        <h3>Any LLM Connector Contract</h3>
        <pre>${escapeHtml(JSON.stringify({
          claim: connector.claim,
          hard_requirement: connector.hard_requirement,
          modes: (connector.integration_modes || []).map((item) => ({ mode: item.mode, status: item.status })),
        }, null, 2))}</pre>
      </article>
    </div>

    <article class="detail-card">
      <h3>Governance Evidence Map</h3>
      <pre>${escapeHtml(JSON.stringify(governance.frameworks || {}, null, 2))}</pre>
    </article>

    <article class="detail-card">
      <h3>Pack Artifacts</h3>
      <div class="artifacts-list">${Object.entries(artifacts).map(([name, item]) => `
        <article class="artifact-row">
          <h3>${escapeHtml(name)}</h3>
          <p><code>${escapeHtml(item.path)}</code></p>
          <p>sha=${escapeHtml(shortHash(item.sha256))} bytes=${escapeHtml(item.bytes)}</p>
        </article>
      `).join('')}</div>
    </article>
  `;
}

function renderRaw(snapshot) {
  $('rawJson').textContent = snapshot ? JSON.stringify(snapshot, null, 2) : '';
}

function renderSnapshot(snapshot) {
  state.snapshot = snapshot;
  renderMetricGrid(snapshot);
  renderToolSurface(snapshot);
  renderTimeline(snapshot);
  renderMetricsDetail(snapshot);
  renderLedger(snapshot);
  renderProductCategory(snapshot);
  renderCorrelations(snapshot);
  renderArtifacts(snapshot);
  renderRaw(snapshot);
}

function appendEvent(event) {
  if (event.client_id && event.client_id !== state.clientId) return;
  if (state.activeRequestId && event.request_id && event.request_id !== state.activeRequestId) return;
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

async function loadLatestCategory() {
  try {
    state.latestProductPack = await apiGet('/api/category/latest');
    renderProductCategory(state.snapshot);
  } catch (err) {
    state.latestProductPack = null;
    renderProductCategory(state.snapshot);
  }
}

async function loadLatestRun() {
  state.activeRequestId = newRequestId('inspect');
  $('events').innerHTML = '';
  addChat('user', '<p>Load the latest completed real run and show the source-of-truth readback.</p>');
  try {
    const runs = await apiGet('/api/runs');
    const latest = (runs.runs || []).find((item) => item.complete);
    if (!latest) throw { error_code: 'NO_REAL_RUN', message: 'No completed real run exists under /tmp.' };
    const snapshot = await apiPost('/api/inspect', {
      root: latest.root,
      compile_reward: false,
      client_id: state.clientId,
      request_id: state.activeRequestId,
    });
    renderSnapshot(snapshot);
    addChat('assistant', `<p>Loaded <code>${escapeHtml(latest.root)}</code>. The right pane is reading physical files and SQLite ledger state from that run.</p>`);
  } catch (err) {
    addChat('assistant', `<p><strong>${escapeHtml(err.error_code || 'ERROR')}</strong>: ${escapeHtml(err.message || err)}</p>`);
  }
}

async function submitPrompt(prompt) {
  state.activeRequestId = newRequestId('chat');
  $('events').innerHTML = '';
  addChat('user', `<p>${escapeHtml(prompt || '(empty prompt)')}</p>`);
  try {
    const snapshot = await apiPost('/api/chat', {
      message: prompt,
      compile_reward: true,
      client_id: state.clientId,
      request_id: state.activeRequestId,
    });
    renderSnapshot(snapshot);
    addChat('assistant', `${renderAgentTruth(snapshot)}<p>BlackBoxFix run: <code>${escapeHtml(snapshot.identity.blackbox_run_id)}</code>.</p>`);
  } catch (err) {
    addChat('assistant', `<p><strong>${escapeHtml(err.error_code || 'ERROR')}</strong>: ${escapeHtml(err.message || err)}</p><p>${escapeHtml(err.remediation || '')}</p>`);
  }
}

async function runLive(event) {
  event.preventDefault();
  await submitPrompt($('prompt').value.trim());
}

async function buildCategoryPack() {
  const prompt = 'Build the investor product category pack. Show the model arena, verified outcome leaderboard, failure injection studio, governance evidence map, connector contracts, signed attestation, reward dashboard, and executive view. Audience: AI infrastructure investors. Max runs: 8.';
  $('prompt').value = prompt;
  await submitPrompt(prompt);
}

function setupTabs() {
  document.querySelectorAll('.tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      document.querySelectorAll('.tab').forEach((item) => item.classList.remove('active'));
      document.querySelectorAll('.tab-panel').forEach((item) => item.classList.remove('active'));
      tab.classList.add('active');
      $(`panel-${tab.dataset.tab}`).classList.add('active');
      state.activeTab = tab.dataset.tab;
      if (tab.dataset.tab === 'product') loadLatestCategory();
    });
  });
  const hashTab = location.hash.replace('#', '');
  if (hashTab) {
    const tab = document.querySelector(`.tab[data-tab="${CSS.escape(hashTab)}"]`);
    if (tab) tab.click();
  }
}

function setupEvents() {
  const source = new EventSource(`/api/events?client_id=${encodeURIComponent(state.clientId)}`);
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
  loadLatestCategory();
  $('refreshHealth').addEventListener('click', refreshHealth);
  $('loadLatest').addEventListener('click', loadLatestRun);
  $('buildCategory').addEventListener('click', buildCategoryPack);
  $('chatForm').addEventListener('submit', runLive);
  document.querySelectorAll('.prompt-chip').forEach((button) => {
    button.addEventListener('click', () => {
      $('prompt').value = button.dataset.prompt || '';
      $('prompt').focus();
    });
  });
});
