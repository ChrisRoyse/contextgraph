#!/usr/bin/env python3
"""Talk with Shakespeare — production chat dashboard.

Zero external dependencies beyond what's already installed.
Single Python file serving a modern glassmorphism chat UI.

    python3 scripts/shakespeare_chat_dashboard.py [--port 8765]
"""
from __future__ import annotations
import argparse, http.server, json, sys, threading, time, urllib.request
from pathlib import Path

SYSTEM = (
    "You are William Shakespeare. Write ONLY in early modern English — use thee, "
    "thou, thy, thine, hast, dost, doth, ye, prithee, forsooth, hark, wherefore, "
    "methinks, 'tis, 'twas, and similar period vocabulary. Use iambic rhythm where "
    "natural. Draw imagery from nature, stars, seasons, love, death, honour, and "
    "kingship. Write verse OR prose in Shakespeare's voice — NOT stage directions, "
    "NOT character names, NOT scene headers. Just beautiful Shakespearean writing "
    "about the topic the user asks about. Keep responses under 100 words."
)
INFERENCE_URL = "http://127.0.0.1:8766"  # Custom LoRA inference server

# Memory management constants
MAX_RECENT_EXCHANGES = 4   # Keep last 4 user+assistant pairs verbatim
SUMMARY_TRIGGER = 6        # Start summarizing after 6 messages (3 exchanges)


def _summarize_context(messages: list[dict]) -> str:
    """Build a compact summary of conversation topics from older messages.

    Returns a single string that captures WHAT was discussed without
    consuming the full token budget of the original messages.
    """
    topics = []
    for msg in messages:
        content = msg.get("content", "")
        role = msg.get("role", "")
        if role == "user":
            # Extract the core topic from user prompts
            short = content[:80].replace("\n", " ").strip()
            topics.append(f"- The user asked: \"{short}\"")
        elif role == "assistant":
            # Extract the first line of each response as a topic anchor
            first_line = content.split("\n")[0][:60].strip()
            if first_line:
                topics.append(f"  Shakespeare replied about: \"{first_line}...\"")
    return "\n".join(topics[-12:])  # Cap at 12 topic lines


def build_context_window(full_history: list[dict]) -> list[dict]:
    """Build an optimized context window from the full conversation history.

    Architecture:
    1. System prompt (always present, always first)
    2. If history is short (≤ SUMMARY_TRIGGER): send everything
    3. If history is long:
       a. First exchange (2 messages) — so model remembers conversation origin
       b. Compact summary of middle turns — topics only, not full text
       c. Last MAX_RECENT_EXCHANGES exchanges — full verbatim for coherence

    This gives the model:
    - Memory of what the conversation started with (HP-08 fix)
    - Awareness of all topics discussed (no lost context)
    - Full recent context for coherent follow-ups
    - Short enough that the LoRA stays dominant (EC-01 fix)
    """
    if len(full_history) <= SUMMARY_TRIGGER:
        return [{"role": "system", "content": SYSTEM}] + full_history

    # Split into: first exchange, middle, recent
    first_exchange = full_history[:2]  # First user + first assistant
    recent_count = MAX_RECENT_EXCHANGES * 2  # 4 exchanges = 8 messages
    recent = full_history[-recent_count:]
    middle = full_history[2:-recent_count] if len(full_history) > 2 + recent_count else []

    context_messages = [{"role": "system", "content": SYSTEM}]

    # First exchange — verbatim
    context_messages.extend(first_exchange)

    # Middle — summarized into a single system message
    if middle:
        summary = _summarize_context(middle)
        context_messages.append({
            "role": "system",
            "content": f"[Prior conversation summary — the following topics were discussed between the first exchange and the recent messages:]\n{summary}"
        })

    # Recent exchanges — verbatim
    context_messages.extend(recent)

    return context_messages


class Engine:
    def __init__(self):
        self._lock = threading.Lock()

    def respond(self, messages):
        t0 = time.time()
        with self._lock:
            result = self._generate(messages)
        gen_ms = (time.time() - t0) * 1000.0
        reply = result.get("content", "")
        tokens = result.get("tokens", 0)
        tps = result.get("tokens_per_sec", 0)

        rec = {
            "attempt": 1, "reply": reply, "temperature": 0.8,
            "gen_ms": round(gen_ms, 1), "score_ms": 0,
            "verdict": "LORA", "verdict_long": f"Shakespeare LoRA — {tokens} tokens at {tps} tok/s",
            "top_work": "LoRA-trained", "top_score": None,
            "top_in_p95": False,
            "second_work": None, "second_score": None,
            "spread": None, "per_embedder": [],
        }
        return {"final": rec, "attempts": [rec]}

    def _generate(self, messages):
        context = build_context_window(messages)
        body = {
            "messages": context,
            "temperature": 0.8,
            "max_tokens": 300,
            "repetition_penalty": 1.2,
        }
        req = urllib.request.Request(f"{INFERENCE_URL}/api/generate",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"}, method="POST")
        with urllib.request.urlopen(req, timeout=300) as resp:
            return json.loads(resp.read())


HTML = r'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>Talk with Shakespeare — Context Graph</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Playfair+Display:ital,wght@0,700;1,400&display=swap" rel="stylesheet">
<style>
*{box-sizing:border-box;margin:0;padding:0}
:root{
  --bg:#0a0a1a;
  --bg2:#0f0f2a;
  --surface:rgba(20,15,50,0.6);
  --surface2:rgba(30,20,70,0.4);
  --glass:rgba(120,80,255,0.08);
  --glass-border:rgba(120,80,255,0.15);
  --purple:#7c3aed;
  --purple-light:#a78bfa;
  --purple-glow:rgba(124,58,237,0.3);
  --purple-subtle:rgba(124,58,237,0.1);
  --text:#f0eef5;
  --text2:#a8a3b8;
  --text3:#6b6580;
  --green:#34d399;
  --green-bg:rgba(52,211,153,0.12);
  --yellow:#fbbf24;
  --yellow-bg:rgba(251,191,36,0.12);
  --red:#f87171;
  --red-bg:rgba(248,113,113,0.12);
  --mono:'Inter',system-ui,sans-serif;
  --display:'Playfair Display',Georgia,serif;
  --col:52rem;
}
html{height:100%;background:var(--bg)}
body{
  min-height:100%;display:flex;flex-direction:column;
  font-family:var(--mono);color:var(--text);
  background:radial-gradient(ellipse 80% 60% at 50% 0%,rgba(124,58,237,0.12),transparent 70%),
             radial-gradient(ellipse 60% 40% at 80% 100%,rgba(80,40,200,0.08),transparent 60%),
             var(--bg);
}

/* ─── Header ─── */
header{
  padding:16px 32px;display:flex;align-items:center;justify-content:space-between;
  border-bottom:1px solid var(--glass-border);
  backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px);
  background:rgba(10,10,26,0.8);position:sticky;top:0;z-index:20;
}
.logo{display:flex;align-items:center;gap:10px}
.logo-icon{width:32px;height:32px;border-radius:8px;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  display:flex;align-items:center;justify-content:center;font-size:16px}
.logo-text{font-family:var(--display);font-size:18px;font-weight:700;color:var(--text)}
.header-pills{display:flex;gap:8px;align-items:center}
.pill{font-size:11px;padding:4px 10px;border-radius:20px;
  background:var(--glass);border:1px solid var(--glass-border);color:var(--text2)}
.pill-live{background:var(--green-bg);border-color:rgba(52,211,153,0.3);color:var(--green)}
.pill-live::before{content:"";display:inline-block;width:6px;height:6px;
  border-radius:50%;background:var(--green);margin-right:5px;
  animation:blink 2s infinite}

/* ─── Chat ─── */
#chat{
  flex:1;overflow-y:auto;padding:32px 24px 16px;
  display:flex;flex-direction:column;gap:24px;
  overscroll-behavior-y:contain;
}
#chat::-webkit-scrollbar{width:5px}
#chat::-webkit-scrollbar-thumb{background:rgba(124,58,237,0.3);border-radius:3px}

.turn{max-width:var(--col);width:100%;margin:0 auto}

/* User message */
.turn-user{display:flex;justify-content:flex-end}
.bubble-user{
  background:linear-gradient(135deg,rgba(124,58,237,0.2),rgba(80,40,200,0.15));
  border:1px solid rgba(124,58,237,0.25);
  padding:14px 20px;border-radius:20px 20px 6px 20px;
  max-width:75%;font-size:15px;line-height:1.6;
}

/* AI message */
.turn-ai{display:flex;flex-direction:column;gap:8px}
.bubble-ai{
  background:var(--surface);
  border:1px solid var(--glass-border);
  backdrop-filter:blur(12px);-webkit-backdrop-filter:blur(12px);
  padding:20px 24px;border-radius:20px 20px 20px 6px;
  font-family:var(--display);font-size:16px;line-height:1.7;
  font-style:italic;color:var(--text);
}

/* Guard badge bar */
.guard-bar{
  display:flex;flex-wrap:wrap;gap:6px;align-items:center;
  padding:0 4px;
}
.badge{
  font-family:var(--mono);font-size:11px;font-weight:600;
  padding:3px 10px;border-radius:20px;text-transform:uppercase;letter-spacing:0.5px;
}
.badge-pass{background:var(--green-bg);color:var(--green);border:1px solid rgba(52,211,153,0.25)}
.badge-warn{background:var(--yellow-bg);color:var(--yellow);border:1px solid rgba(251,191,36,0.25)}
.badge-fail{background:var(--red-bg);color:var(--red);border:1px solid rgba(248,113,113,0.25)}
.stat{font-family:var(--mono);font-size:11px;color:var(--text3);
  padding:3px 8px;background:var(--glass);border-radius:12px;border:1px solid var(--glass-border)}

/* Collapsible embedder details */
.details-wrap{
  background:var(--surface2);border:1px solid var(--glass-border);
  border-radius:14px;overflow:hidden;
}
.details-wrap summary{
  cursor:pointer;font-size:12px;color:var(--text3);padding:10px 16px;
  user-select:none;list-style:none;display:flex;align-items:center;gap:6px;
}
.details-wrap summary::before{content:"▸";transition:transform .2s}
.details-wrap[open] summary::before{transform:rotate(90deg)}
.details-wrap summary::-webkit-details-marker{display:none}
.details-inner{padding:0 16px 14px}

/* Embedder strip — bar chart */
.emb-chart{display:flex;gap:3px;height:48px;align-items:flex-end;margin:8px 0}
.emb-col{flex:1;min-width:16px;display:flex;flex-direction:column;align-items:center;gap:2px}
.emb-bar{width:100%;border-radius:3px 3px 0 0;transition:height .4s ease}
.emb-label{font-size:8px;color:var(--text3);font-family:var(--mono)}
.emb-val{font-size:8px;font-family:var(--mono)}
.emb-high{background:linear-gradient(180deg,var(--green),rgba(52,211,153,0.4))}
.emb-high .emb-val{color:var(--green)}
.emb-mid{background:linear-gradient(180deg,var(--purple-light),rgba(167,139,250,0.4))}
.emb-mid .emb-val{color:var(--purple-light)}
.emb-low{background:linear-gradient(180deg,var(--red),rgba(248,113,113,0.3))}
.emb-low .emb-val{color:var(--red)}
.emb-zero{background:rgba(107,101,128,0.2)}
.emb-zero .emb-val{color:var(--text3)}

.meta-row{display:grid;grid-template-columns:auto 1fr;gap:2px 14px;
  font-size:11px;font-family:var(--mono);color:var(--text2);margin-top:8px}
.meta-row .k{color:var(--purple-light)}

/* Attempt trail */
.trail{font-size:10px;font-family:var(--mono);color:var(--text3);margin-top:6px}

/* Typing */
.typing{font-family:var(--display);font-style:italic;color:var(--text3);font-size:15px;padding:4px 0}
@keyframes blink{0%,80%,100%{opacity:.2}40%{opacity:1}}
.dot{animation:blink 1.4s infinite}.dot:nth-child(2){animation-delay:.2s}.dot:nth-child(3){animation-delay:.4s}

/* ─── Welcome ─── */
.welcome{text-align:center;padding:80px 20px 40px;max-width:560px;margin:auto}
.welcome-glow{
  width:120px;height:120px;margin:0 auto 24px;border-radius:24px;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  box-shadow:0 0 80px var(--purple-glow),0 0 160px rgba(124,58,237,0.15);
  display:flex;align-items:center;justify-content:center;font-size:56px;
}
.welcome h2{font-family:var(--display);font-size:28px;color:var(--text);margin-bottom:12px;font-weight:700}
.welcome p{font-size:14px;color:var(--text2);line-height:1.7;margin-bottom:28px}
.suggestions{display:flex;flex-wrap:wrap;gap:10px;justify-content:center}
.sug{
  font-size:13px;padding:10px 16px;
  background:var(--glass);border:1px solid var(--glass-border);
  border-radius:12px;cursor:pointer;color:var(--text2);
  transition:all .2s;
}
.sug:hover{border-color:var(--purple);color:var(--text);background:var(--purple-subtle);
  box-shadow:0 0 20px rgba(124,58,237,0.15)}

/* ─── Input ─── */
#form-wrap{
  padding:16px 24px 20px;
  background:rgba(10,10,26,0.9);border-top:1px solid var(--glass-border);
  backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px);
}
#form{max-width:var(--col);margin:0 auto;display:flex;align-items:flex-end;gap:10px}
#input{
  flex:1;resize:none;min-height:48px;max-height:200px;
  padding:14px 18px;border-radius:16px;
  border:1px solid var(--glass-border);
  background:var(--surface);color:var(--text);
  font:15px var(--mono);line-height:1.5;outline:none;overflow-y:auto;
  backdrop-filter:blur(8px);transition:border-color .2s;
}
#input:focus{border-color:var(--purple);box-shadow:0 0 20px rgba(124,58,237,0.1)}
#input::placeholder{color:var(--text3)}
#sendbtn{
  width:48px;height:48px;border-radius:14px;flex-shrink:0;border:none;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  color:#fff;cursor:pointer;font-size:20px;
  display:flex;align-items:center;justify-content:center;
  transition:all .2s;box-shadow:0 4px 20px rgba(124,58,237,0.3);
}
#sendbtn:disabled{opacity:.3;cursor:not-allowed;box-shadow:none}
#sendbtn:hover:not(:disabled){transform:translateY(-1px);box-shadow:0 6px 30px rgba(124,58,237,0.4)}

/* ─── Responsive ─── */
@media(min-width:2560px){:root{--col:60rem}}
@media(min-width:3440px){:root{--col:68rem}}
@media(max-width:767px){
  :root{--col:100%}
  header{padding:12px 16px}
  .header-pills{display:none}
  #chat{padding:16px 12px 8px;gap:16px}
  .bubble-user{max-width:90%}
  .bubble-ai{padding:14px 16px;font-size:15px}
  .emb-chart{overflow-x:auto;-webkit-overflow-scrolling:touch}
  .emb-col{min-width:26px;flex:0 0 26px}
  #sendbtn{width:48px;height:48px}
  .welcome{padding:40px 16px 20px}
  .welcome-glow{width:80px;height:80px;font-size:36px;border-radius:16px}
  .welcome h2{font-size:22px}
  .sug{font-size:12px;padding:8px 12px}
}
</style>
</head>
<body>
<header>
  <div class="logo">
    <div class="logo-icon">&#x1F3AD;</div>
    <span class="logo-text">Talk with Shakespeare</span>
  </div>
  <div class="header-pills">
    <span class="pill pill-live">Live</span>
    <span class="pill">Gemma 4 E4B</span>
    <span class="pill">Shakespeare LoRA</span>
    <span class="pill">SFT + DPO trained</span>
  </div>
</header>

<div id="chat">
  <div class="welcome" id="welcome">
    <div class="welcome-glow">&#x1F3AD;</div>
    <h2>Speak with William Shakespeare</h2>
    <p>This model was fine-tuned on Shakespeare's complete works using Context Graph's 13-embedder decomposition pipeline &mdash; 994 SFT examples with Gemma-generated prompts + 12,567 DPO contrastive pairs mined from cross-embedder anomalies.</p>
    <div class="suggestions">
      <span class="sug" onclick="useSug(this)">Tell me of love on a summer's morn</span>
      <span class="sug" onclick="useSug(this)">What thinkest thou of ambition?</span>
      <span class="sug" onclick="useSug(this)">My uncle hath stolen the throne &mdash; advise me</span>
      <span class="sug" onclick="useSug(this)">Speak of the stars and fate</span>
    </div>
  </div>
</div>

<div id="form-wrap">
  <form id="form" onsubmit="handleSend(event)">
    <textarea id="input" rows="1" placeholder="Speak unto Shakespeare..."
      onkeydown="if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();handleSend(event)}"></textarea>
    <button id="sendbtn" type="submit" aria-label="Send">&#10148;</button>
  </form>
</div>

<script>
const chatEl=document.getElementById('chat'),inputEl=document.getElementById('input'),
      sendBtn=document.getElementById('sendbtn');
const hist=[];
const EMB=['E1','E2','E3','E4','E5','E6','E7','E8','E9','E10','E11','E12','E13'];

function esc(s){return(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')}
function useSug(el){inputEl.value=el.textContent;inputEl.focus()}
function scrollDown(){chatEl.scrollTo({top:chatEl.scrollHeight,behavior:'smooth'})}

function addUser(text){
  document.getElementById('welcome')?.remove();
  const d=document.createElement('div');d.className='turn turn-user';
  d.innerHTML='<div class="bubble-user">'+esc(text)+'</div>';
  chatEl.appendChild(d);scrollDown();
}

function addTyping(){
  const d=document.createElement('div');d.className='turn turn-ai';d.id='typing';
  d.innerHTML='<div class="bubble-ai"><span class="typing"><span class="dot">&#9679;</span> <span class="dot">&#9679;</span> <span class="dot">&#9679;</span> Shakespeare is composing&hellip;</span></div>';
  chatEl.appendChild(d);scrollDown();
}

function addAI(data){
  document.getElementById('typing')?.remove();
  const f=data.final;
  const vc=f.verdict==='REJECTED'||f.verdict==='EMPTY'?'fail':f.verdict==='ARCHAIC'?'warn':'pass';
  const d=document.createElement('div');d.className='turn turn-ai';

  // Bubble
  let h='<div class="bubble-ai">&ldquo;'+esc(f.reply||'(Silence.)')+'&rdquo;</div>';

  // Guard bar
  h+='<div class="guard-bar">';
  h+='<span class="badge badge-'+vc+'">'+esc(f.verdict)+'</span>';
  if(f.top_work)h+='<span class="stat">'+esc(f.top_work)+' &middot; '+(f.top_score??'')+'</span>';
  if(f.gen_ms)h+='<span class="stat">gen '+f.gen_ms+'ms</span>';
  if(f.score_ms)h+='<span class="stat">guard '+f.score_ms+'ms</span>';
  h+='<span class="stat">#'+f.attempt+'/'+data.attempts.length+'</span>';
  h+='</div>';

  // Collapsible details
  h+='<details class="details-wrap"><summary>Embedder details &amp; scores</summary><div class="details-inner">';

  // Bar chart
  if(f.per_embedder?.length){
    h+='<div class="emb-chart">';
    for(let i=0;i<f.per_embedder.length;i++){
      const v=f.per_embedder[i],pct=Math.max(2,Math.min(100,Math.abs(v)*100));
      const cls=v>0.7?'high':v>0.4?'mid':v>0.01?'low':'zero';
      h+='<div class="emb-col emb-'+cls+'"><span class="emb-val">'+v.toFixed(2)+'</span>';
      h+='<div class="emb-bar" style="height:'+pct+'%"></div>';
      h+='<span class="emb-label">'+EMB[i]+'</span></div>';
    }
    h+='</div>';
  }

  h+='<div class="meta-row">';
  h+='<span class="k">verdict</span><span>'+esc(f.verdict_long||f.verdict)+'</span>';
  h+='<span class="k">top</span><span>'+esc(f.top_work||'')+' ('+((f.top_score??''))+(f.top_in_p95?' &middot; in p95':'')+')</span>';
  h+='<span class="k">runner-up</span><span>'+esc(f.second_work||'')+' ('+((f.second_score??''))+') spread +'+((f.spread??0))+'</span>';
  h+='<span class="k">timing</span><span>attempt '+f.attempt+' &middot; temp '+(f.temperature??'')+' &middot; gen '+f.gen_ms+'ms &middot; guard '+f.score_ms+'ms</span>';
  h+='</div>';

  if(data.attempts.length>1){
    h+='<div class="trail">'+data.attempts.length+' attempts: ';
    for(const a of data.attempts){
      const c2=a.verdict==='REJECTED'||a.verdict==='EMPTY'?'badge-fail':a.verdict==='ARCHAIC'?'badge-warn':'badge-pass';
      h+='<span class="badge '+c2+'" style="font-size:9px">'+a.verdict+'</span> ';
    }
    h+='</div>';
  }
  h+='</div></details>';

  d.innerHTML=h;chatEl.appendChild(d);scrollDown();
}

async function handleSend(e){
  e.preventDefault();
  const text=inputEl.value.trim();if(!text)return;
  inputEl.value='';inputEl.style.height='auto';
  sendBtn.disabled=true;
  addUser(text);
  hist.push({role:'user',content:text});
  addTyping();
  try{
    const r=await fetch('/api/chat',{method:'POST',
      headers:{'Content-Type':'application/json'},
      body:JSON.stringify({messages:hist})});
    if(!r.ok)throw new Error('HTTP '+r.status+': '+await r.text());
    const data=await r.json();
    addAI(data);
    if(data.final.reply)hist.push({role:'assistant',content:data.final.reply});
  }catch(err){
    document.getElementById('typing')?.remove();
    const d=document.createElement('div');d.className='turn turn-ai';
    d.innerHTML='<div class="bubble-ai" style="color:var(--red);font-style:normal">Error: '+esc(err.message)+'</div>';
    chatEl.appendChild(d);
  }finally{sendBtn.disabled=false;inputEl.focus()}
}
inputEl.addEventListener('input',()=>{inputEl.style.height='auto';inputEl.style.height=Math.min(inputEl.scrollHeight,200)+'px'});
</script>
</body>
</html>
'''


class Handler(http.server.BaseHTTPRequestHandler):
    engine=None
    def log_message(self,f,*a):sys.stderr.write(f"[shakespeare] {f%a}\n")
    def _json(self,code,obj):
        b=json.dumps(obj).encode()
        self.send_response(code);self.send_header("Content-Type","application/json")
        self.send_header("Content-Length",str(len(b)));self.end_headers();self.wfile.write(b)
    def do_GET(self):
        if self.path in("/","/index.html"):
            b=HTML.encode();self.send_response(200)
            self.send_header("Content-Type","text/html; charset=utf-8")
            self.send_header("Content-Length",str(len(b)));self.end_headers();self.wfile.write(b)
        elif self.path=="/health":self._json(200,{"ok":True,"constellations":len(self.engine.cons)})
        elif self.path=="/favicon.ico":self.send_response(204);self.end_headers()
        else:self.send_error(404)
    def do_POST(self):
        if self.path!="/api/chat":self.send_error(404);return
        raw=self.rfile.read(int(self.headers.get("Content-Length",0)))
        try:p=json.loads(raw);msgs=p["messages"]
        except(json.JSONDecodeError,KeyError)as e:self._json(400,{"error":str(e)});return
        try:self._json(200,self.engine.respond(msgs))
        except Exception as e:
            sys.stderr.write(f"[shakespeare] ERROR: {e}\n");self._json(500,{"error":str(e)})


def main():
    ap=argparse.ArgumentParser();ap.add_argument("--port",type=int,default=8765)
    args=ap.parse_args()
    # Verify inference server is running
    print(f"[shakespeare] checking inference server at {INFERENCE_URL} ...",file=sys.stderr)
    try:
        r=urllib.request.urlopen(f"{INFERENCE_URL}/health",timeout=5)
        health=json.loads(r.read())
        print(f"[shakespeare] inference server OK: {health.get('vram_gb',0)}GB VRAM, LoRA loaded",file=sys.stderr)
    except Exception as e:
        raise RuntimeError(f"Inference server not running at {INFERENCE_URL}. Start it: python3 scripts/shakespeare_inference_server.py --port 8766") from e
    Handler.engine=Engine()
    httpd=http.server.ThreadingHTTPServer(("127.0.0.1",args.port),Handler)
    print(f"[shakespeare] READY — http://127.0.0.1:{args.port}",file=sys.stderr)
    try:httpd.serve_forever()
    except KeyboardInterrupt:print("\n[shakespeare] shutdown",file=sys.stderr)
    finally:httpd.server_close();rpc.close()

if __name__=="__main__":main()
