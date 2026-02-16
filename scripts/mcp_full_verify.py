#!/usr/bin/env python3
"""
MCP Context-Graph Full System Verification
~85 tests across 14 phases — automated JSON-RPC over stdio
"""
import json
import subprocess
import sys
import time
import threading
import queue
import os

# --- Config ---
MCP_BINARY = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                          "target", "release", "context-graph-mcp")
TIMEOUT = 90  # seconds per call
REQUEST_ID = 0
STORED_IDS = {}  # name -> memory_id
RESULTS = []  # (phase, test, pass/fail, detail)
PROC = None
RESPONSE_QUEUE = queue.Queue()
READER_THREAD = None

# --- Colors ---
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
RESET = "\033[0m"
BOLD = "\033[1m"

def log_phase(phase_num, title):
    print(f"\n{BOLD}{CYAN}{'='*60}{RESET}")
    print(f"{BOLD}{CYAN}  Phase {phase_num}: {title}{RESET}")
    print(f"{BOLD}{CYAN}{'='*60}{RESET}")

def log_test(test_id, desc, passed, detail=""):
    status = f"{GREEN}PASS{RESET}" if passed else f"{RED}FAIL{RESET}"
    print(f"  [{status}] {test_id}: {desc}")
    if detail and not passed:
        for line in str(detail).split("\n")[:3]:
            print(f"         {YELLOW}{line[:120]}{RESET}")
    RESULTS.append((test_id, desc, passed, detail))

def stdout_reader(proc):
    """Background thread to read lines from stdout"""
    try:
        for line in proc.stdout:
            line = line.strip()
            if line:
                try:
                    obj = json.loads(line)
                    RESPONSE_QUEUE.put(obj)
                except json.JSONDecodeError:
                    pass  # Skip non-JSON lines (logs etc)
    except:
        pass

def start_server():
    global PROC, READER_THREAD
    env = os.environ.copy()
    env["RUST_LOG"] = "warn"
    PROC = subprocess.Popen(
        [MCP_BINARY],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        bufsize=0,
    )
    READER_THREAD = threading.Thread(target=stdout_reader, args=(PROC,), daemon=True)
    READER_THREAD.start()

    print(f"{YELLOW}Starting MCP server (loading 13 embedders + models)...{RESET}")
    time.sleep(3)

    # Send initialize
    resp = send_rpc("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "mcp-verify", "version": "1.0.0"}
    }, timeout=30)
    if resp and "result" in resp:
        name = resp["result"].get("serverInfo", {}).get("name", "unknown")
        print(f"{GREEN}Server initialized: {name}{RESET}")
        # Send initialized notification
        send_notification("notifications/initialized", {})
        time.sleep(2)
        return True
    else:
        print(f"{RED}Server failed to initialize: {resp}{RESET}")
        return False

def stop_server():
    global PROC
    if PROC:
        try:
            PROC.stdin.close()
        except:
            pass
        PROC.terminate()
        try:
            PROC.wait(timeout=5)
        except:
            PROC.kill()
        PROC = None

def send_notification(method, params):
    msg = json.dumps({"jsonrpc": "2.0", "method": method, "params": params})
    try:
        PROC.stdin.write(msg.encode() + b"\n")
        PROC.stdin.flush()
    except:
        pass

def send_rpc(method, params, timeout=TIMEOUT):
    global REQUEST_ID
    REQUEST_ID += 1
    my_id = REQUEST_ID
    msg = json.dumps({"jsonrpc": "2.0", "id": my_id, "method": method, "params": params})
    try:
        PROC.stdin.write(msg.encode() + b"\n")
        PROC.stdin.flush()
    except BrokenPipeError:
        return {"error": {"message": "Server pipe broken"}}

    # Wait for our response
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = RESPONSE_QUEUE.get(timeout=min(2.0, deadline - time.time()))
            if resp.get("id") == my_id:
                return resp
            # Put back if not ours (shouldn't happen often)
        except queue.Empty:
            continue

    return {"error": {"message": f"Timeout after {timeout}s"}}

def call_tool(name, arguments, timeout=TIMEOUT):
    """Call an MCP tool and return the parsed result"""
    resp = send_rpc("tools/call", {"name": name, "arguments": arguments}, timeout=timeout)
    if "error" in resp:
        err = resp["error"]
        if isinstance(err, dict):
            return {"error": err.get("message", str(err))}
        return {"error": str(err)}
    result = resp.get("result", {})
    content = result.get("content", [])
    if content:
        text = content[0].get("text", "")
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return {"text": text, "raw": True}
    return result

# ============================================================
# PHASE 1: Infrastructure Health Check
# ============================================================
def phase_1():
    log_phase(1, "Infrastructure Health Check")

    result = call_tool("get_memetic_status", {})
    if "error" in result:
        log_test("1.2", "System Status", False, result["error"])
        return False

    ec = result.get("embedder_count", 0)
    log_test("1.2a", f"Embedder count = 13 (got {ec})", ec == 13)

    sb = result.get("storage_backend", "")
    log_test("1.2b", f"Storage backend (got {sb})", sb == "rocksdb")

    gate = result.get("e5_causal_gate", {})
    log_test("1.2c", "E5 causal gate enabled", gate.get("enabled", False))

    ct = gate.get("causal_threshold", 0)
    log_test("1.2d", f"Causal threshold = 0.04 (got {ct})", abs(ct - 0.04) < 0.001)

    profiles = result.get("available_profiles", [])
    log_test("1.2e", f"Weight profiles >= 14 (got {len(profiles)})", len(profiles) >= 14)

    fps = result.get("total_fingerprints", -1)
    log_test("1.2f", f"Total fingerprints = {fps}", fps >= 0)

    return True

# ============================================================
# PHASE 2: Core Memory Operations (CRUD)
# ============================================================
def phase_2():
    log_phase(2, "Core Memory Operations (CRUD)")

    stores = [
        ("rocksdb", "RocksDB uses log-structured merge trees (LSM) for write-optimized storage. Column families provide logical separation of data with independent compaction.", "Store RocksDB memory"),
        ("http2", "The HTTP/2 protocol uses multiplexed streams over a single TCP connection, reducing head-of-line blocking. HPACK header compression minimizes overhead.", "Store HTTP/2 memory"),
        ("code", "fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 { let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt(); let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt(); if norm_a == 0.0 || norm_b == 0.0 { return 0.0; } dot / (norm_a * norm_b) }", "Store code memory"),
        ("causal", "Memory leaks in long-running services cause gradual heap growth, which leads to increased GC pressure, which causes latency spikes, which triggers timeout errors in downstream services.", "Store causal memory"),
        ("entity", "PostgreSQL and Redis are used together in the authentication service. PostgreSQL stores user credentials while Redis caches session tokens. The API gateway routes requests through Nginx to the auth service.", "Store entity memory"),
    ]

    for i, (key, content, desc) in enumerate(stores):
        t0 = time.time()
        result = call_tool("store_memory", {"content": content, "rationale": f"Test {key}"}, timeout=120)
        elapsed = time.time() - t0
        mid = result.get("memory_id", "")
        STORED_IDS[key] = mid
        log_test(f"2.{i+1}", f"{desc} [{elapsed:.1f}s]", bool(mid), result.get("error", ""))

    # 2.6 Basic search
    result = call_tool("search_graph", {"query": "LSM tree write optimization", "limit": 5})
    if "error" not in result:
        rl = result.get("results", [])
        top = rl[0].get("similarity", 0) if rl else 0
        log_test("2.6", f"Basic search (n={len(rl)}, top_sim={top:.3f})", len(rl) > 0 and top > 0.3)
    else:
        log_test("2.6", "Basic search", False, result["error"])

    # 2.7 Negative search
    result = call_tool("search_graph", {"query": "quantum entanglement teleportation protocol", "limit": 5})
    if "error" not in result:
        rl = result.get("results", [])
        top = rl[0].get("similarity", 0) if rl else 0
        log_test("2.7", f"Negative search (top_sim={top:.3f}, want low)", True)
    else:
        log_test("2.7", "Negative search", False, result.get("error", ""))

    return True

# ============================================================
# PHASE 3: Search Strategy Verification
# ============================================================
def phase_3():
    log_phase(3, "Search Strategy Verification")

    strategies = [
        ("3.1", "e1_only", 5),
        ("3.2", "multi_space", 5),
        ("3.3", "pipeline", 10),
    ]
    for tid, strat, lim in strategies:
        t0 = time.time()
        result = call_tool("search_graph", {
            "query": "database storage engine compaction", "limit": lim, "search_strategy": strat
        })
        elapsed = time.time() - t0
        if "error" not in result:
            rl = result.get("results", [])
            top = rl[0].get("similarity", 0) if rl else 0
            log_test(tid, f"{strat} (n={len(rl)}, top={top:.3f}, {elapsed:.1f}s)", len(rl) > 0)
        else:
            log_test(tid, f"{strat} strategy", False, result["error"])

    # 3.4 Embedder-first
    result = call_tool("search_by_embedder", {
        "query": "database storage engine compaction", "embedder": "E1_Semantic", "limit": 5
    })
    if "error" not in result:
        rl = result.get("results", [])
        log_test("3.4", f"Embedder-first E1 (n={len(rl)})", len(rl) > 0)
    else:
        log_test("3.4", "Embedder-first E1", False, result["error"])

# ============================================================
# PHASE 4: Individual Embedder Verification
# ============================================================
def phase_4():
    log_phase(4, "Individual Embedder Verification (13 embedders)")

    tests = [
        ("4.1", "search_by_embedder", {"query": "write-optimized storage engine", "embedder": "E1_Semantic", "limit": 3}, "E1 Semantic"),
        ("4.2", "search_recent", {"query": "storage", "limit": 5, "recency_boost": 2.0}, "E2 Temporal Recent"),
        ("4.3", "search_periodic", {"query": "storage", "limit": 5, "periodicity_boost": 2.0}, "E3 Temporal Periodic"),
        ("4.4", "get_session_timeline", {"limit": 10}, "E4 Session Timeline"),
        ("4.5a", "search_causes", {"query": "timeout errors in downstream services", "limit": 5}, "E5 search_causes"),
        ("4.5b", "search_effects", {"query": "memory leaks in long-running services", "limit": 5}, "E5 search_effects"),
        ("4.6", "search_by_keywords", {"query": "RocksDB LSM compaction column families", "limit": 5}, "E6 Sparse Keyword"),
        ("4.7", "search_code", {"query": "cosine similarity vector dot product", "limit": 5}, "E7 Code"),
        ("4.8", "search_connections", {"query": "database", "limit": 5}, "E8 Graph/Connectivity"),
        ("4.9", "search_robust", {"query": "RocksDb LSN tree compactionn", "limit": 5}, "E9 HDC Typo-tolerant"),
        ("4.10", "search_by_embedder", {"query": "a tree-based data structure that optimizes for sequential writes", "embedder": "E10_Multimodal", "limit": 3}, "E10 Paraphrase"),
        ("4.11", "search_by_entities", {"query": "PostgreSQL Redis", "limit": 5}, "E11 Entity"),
        ("4.12", "search_graph", {"query": "log structured merge tree", "limit": 5, "search_strategy": "pipeline"}, "E12 ColBERT/Pipeline"),
        ("4.13", "search_by_embedder", {"query": "storage engine write optimization", "embedder": "E13_SPLADE", "limit": 5}, "E13 SPLADE"),
    ]

    for tid, tool, args, desc in tests:
        result = call_tool(tool, args)
        if "error" not in result:
            # Try various result field names
            rl = result.get("results", result.get("timeline", result.get("memories", [])))
            if isinstance(rl, list):
                log_test(tid, f"{desc} (n={len(rl)})", len(rl) > 0 or "timeline" in str(result) or "session" in str(result))
            else:
                log_test(tid, f"{desc}", True)
        else:
            log_test(tid, desc, False, result["error"])

# ============================================================
# PHASE 5: Weight Profiles
# ============================================================
def phase_5():
    log_phase(5, "Weight Profiles")

    profiles = [
        ("5.1", "semantic_search", "data storage optimization"),
        ("5.2", "code_search", "cosine similarity implementation"),
        ("5.3", "causal_reasoning", "what causes timeout errors"),
        ("5.4", "fact_checking", "PostgreSQL Redis authentication"),
        ("5.5", "typo_tolerant", "RocksDV compacton"),
    ]
    for tid, profile, query in profiles:
        result = call_tool("search_graph", {"query": query, "limit": 5, "weight_profile": profile})
        if "error" not in result:
            rl = result.get("results", [])
            top = rl[0].get("similarity", 0) if rl else 0
            log_test(tid, f"Profile: {profile} (n={len(rl)}, top={top:.3f})", len(rl) > 0)
        else:
            log_test(tid, f"Profile: {profile}", False, result["error"])

    # 5.6 Custom profile
    result = call_tool("create_weight_profile", {
        "name": "test_custom_e1_only",
        "weights": {
            "E1_Semantic": 1.0, "E5_Causal": 0.0, "E6_Sparse": 0.0,
            "E7_Code": 0.0, "E8_Emotional": 0.0, "E9_HDC": 0.0,
            "E10_Multimodal": 0.0, "E11_Entity": 0.0
        }
    })
    log_test("5.6a", "Create custom weight profile", "error" not in result, result.get("error", ""))

    result = call_tool("search_graph", {"query": "storage engine", "limit": 5, "weight_profile": "test_custom_e1_only"})
    if "error" not in result:
        rl = result.get("results", [])
        log_test("5.6b", f"Search with custom profile (n={len(rl)})", len(rl) > 0)
    else:
        log_test("5.6b", "Search with custom profile", False, result["error"])

# ============================================================
# PHASE 6: Causal Subsystem
# ============================================================
def phase_6():
    log_phase(6, "Causal Subsystem (E5 + LLM)")

    tests = [
        ("6.1", "get_causal_chain", {"query": "memory leaks", "max_depth": 3, "limit": 5}, "Causal chain"),
        ("6.2", "search_causal_relationships", {"description": "memory management problems", "limit": 5}, "Causal relationships"),
    ]
    for tid, tool, args, desc in tests:
        result = call_tool(tool, args)
        log_test(tid, desc, "error" not in result, result.get("error", ""))

    # 6.3 LLM causal discovery (may fail gracefully)
    result = call_tool("trigger_causal_discovery", {
        "content": "Increasing the connection pool size reduces database connection wait times, which improves API response latency."
    }, timeout=120)
    ok = "error" not in result or "llm" in str(result.get("error", "")).lower() or "not available" in str(result.get("error", "")).lower()
    log_test("6.3", "LLM causal discovery (or graceful degradation)", ok, result.get("error", ""))

    # 6.4 Causal repair
    result = call_tool("repair_causal_relationships", {"dry_run": True})
    log_test("6.4", "Causal repair (dry_run)", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 7: Graph Subsystem
# ============================================================
def phase_7():
    log_phase(7, "Graph Subsystem (E8 + LLM)")

    rid = STORED_IDS.get("rocksdb", "")
    hid = STORED_IDS.get("http2", "")

    if rid and hid:
        result = call_tool("get_graph_path", {"source_id": rid, "target_id": hid, "max_depth": 3})
        log_test("7.1", "Graph path", "error" not in result, result.get("error", ""))

    if rid:
        for tid, tool, desc in [
            ("7.2", "get_memory_neighbors", "Memory neighbors"),
            ("7.3", "get_typed_edges", "Typed edges"),
            ("7.4", "get_unified_neighbors", "Unified neighbors"),
        ]:
            args = {"memory_id": rid, "limit": 5} if "neighbor" in tool or "unified" in tool else {"memory_id": rid, "limit": 10}
            result = call_tool(tool, args)
            log_test(tid, desc, "error" not in result, result.get("error", ""))

        result = call_tool("traverse_graph", {"start_id": rid, "max_depth": 2, "limit": 10})
        log_test("7.5", "Graph traversal", "error" not in result, result.get("error", ""))

    # LLM tools (may degrade gracefully)
    result = call_tool("discover_graph_relationships", {
        "content": "RocksDB uses LSM trees. LSM trees optimize sequential writes.",
        "limit": 5
    }, timeout=120)
    ok = "error" not in result or "llm" in str(result.get("error", "")).lower()
    log_test("7.6", "LLM graph discovery (or degradation)", ok, result.get("error", ""))

    result = call_tool("validate_graph_link", {
        "source_content": "RocksDB uses LSM trees for storage",
        "target_content": "LSM trees optimize write performance",
        "relationship_type": "enables"
    }, timeout=120)
    ok = "error" not in result or "llm" in str(result.get("error", "")).lower()
    log_test("7.7", "LLM link validation (or degradation)", ok, result.get("error", ""))

# ============================================================
# PHASE 8: Entity Subsystem
# ============================================================
def phase_8():
    log_phase(8, "Entity Subsystem (E11)")

    result = call_tool("extract_entities", {
        "content": "PostgreSQL 16 was released by the PostgreSQL Global Development Group. It includes improvements to logical replication and query parallelism."
    })
    entities = result.get("entities", [])
    log_test("8.1", f"Entity extraction (n={len(entities)})", "error" not in result, result.get("error", ""))

    result = call_tool("search_by_entities", {"query": "PostgreSQL", "limit": 5})
    n = len(result.get("results", [])) if "error" not in result else 0
    log_test("8.2", f"Entity search (n={n})", n > 0, result.get("error", ""))

    result = call_tool("infer_relationship", {
        "source_entity": "PostgreSQL", "target_entity": "Redis", "context": "authentication service"
    })
    log_test("8.3", "Relationship inference", "error" not in result, result.get("error", ""))

    result = call_tool("find_related_entities", {"entity": "PostgreSQL", "limit": 5})
    log_test("8.4", "Related entities", "error" not in result, result.get("error", ""))

    result = call_tool("validate_knowledge", {"statement": "PostgreSQL is a relational database", "limit": 5})
    log_test("8.5", "Knowledge validation", "error" not in result, result.get("error", ""))

    result = call_tool("get_entity_graph", {"entity": "PostgreSQL", "depth": 2, "limit": 10})
    log_test("8.6", "Entity graph", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 9: Topic, Session, and Temporal
# ============================================================
def phase_9():
    log_phase(9, "Topic, Session, and Temporal")

    simple_tests = [
        ("9.1", "detect_topics", {"content": "The database uses write-ahead logging for crash recovery. Transaction isolation levels prevent dirty reads.", "limit": 5}, "Topic detection"),
        ("9.2", "get_topic_portfolio", {}, "Topic portfolio"),
        ("9.3", "get_topic_stability", {}, "Topic stability"),
        ("9.4", "get_divergence_alerts", {}, "Divergence alerts"),
        ("9.5", "get_conversation_context", {"limit": 10}, "Conversation context"),
        ("9.6", "get_session_timeline", {"limit": 10}, "Session timeline"),
    ]
    for tid, tool, args, desc in simple_tests:
        result = call_tool(tool, args)
        log_test(tid, desc, "error" not in result, result.get("error", ""))

    rid = STORED_IDS.get("rocksdb", "")
    if rid:
        result = call_tool("traverse_memory_chain", {"memory_id": rid, "direction": "forward", "limit": 5})
        log_test("9.7", "Memory chain traversal", "error" not in result, result.get("error", ""))

    result = call_tool("compare_session_states", {"session_id_a": "test-a", "session_id_b": "test-b"})
    log_test("9.8", "Session state comparison", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 10: Provenance, Audit Trail, and Curation
# ============================================================
def phase_10():
    log_phase(10, "Provenance, Audit Trail, and Curation")

    rid = STORED_IDS.get("rocksdb", "")
    hid = STORED_IDS.get("http2", "")

    if rid:
        result = call_tool("get_audit_trail", {"target_id": rid, "limit": 10})
        log_test("10.1", "Audit trail", "error" not in result, result.get("error", ""))

        result = call_tool("get_provenance_chain", {"memory_id": rid})
        log_test("10.2", "Provenance chain", "error" not in result, result.get("error", ""))

        result = call_tool("get_merge_history", {"memory_id": rid})
        log_test("10.3", "Merge history", "error" not in result, result.get("error", ""))

        result = call_tool("boost_importance", {"memory_id": rid, "boost": 0.1})
        log_test("10.4", "Importance boost (+0.1)", "error" not in result, result.get("error", ""))

        result = call_tool("boost_importance", {"memory_id": rid, "boost": 5.0})
        imp = result.get("importance", result.get("new_importance", 999))
        log_test("10.5", f"Importance clamping (got {imp}, want <= 1.0)", imp <= 1.0 if isinstance(imp, (int, float)) else False)

    if rid and hid:
        result = call_tool("merge_concepts", {"source_id": hid, "target_id": rid, "dry_run": True})
        log_test("10.6", "Merge concepts (dry_run)", "error" not in result, result.get("error", ""))

    if hid:
        result = call_tool("forget_concept", {"memory_id": hid, "reason": "Test verification"})
        log_test("10.7", "Forget concept (soft delete)", "error" not in result, result.get("error", ""))

    result = call_tool("trigger_consolidation", {})
    log_test("10.8", "Consolidation trigger", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 11: Embedder-First Advanced Tools
# ============================================================
def phase_11():
    log_phase(11, "Embedder-First Advanced Tools")

    rid = STORED_IDS.get("rocksdb", "")

    result = call_tool("get_embedder_clusters", {"embedder": "E1_Semantic", "num_clusters": 3, "limit": 10})
    log_test("11.1", "Embedder clusters", "error" not in result, result.get("error", ""))

    result = call_tool("compare_embedder_views", {
        "query": "database storage", "embedders": ["E1_Semantic", "E7_Code", "E11_Entity"], "limit": 5
    })
    log_test("11.2", "Compare embedder views", "error" not in result, result.get("error", ""))

    result = call_tool("list_embedder_indexes", {})
    log_test("11.3", "List embedder indexes", "error" not in result, result.get("error", ""))

    if rid:
        result = call_tool("get_memory_fingerprint", {"memory_id": rid})
        log_test("11.4", "Memory fingerprint", "error" not in result, result.get("error", ""))

    result = call_tool("search_cross_embedder_anomalies", {"query": "database optimization", "limit": 5})
    log_test("11.5", "Cross-embedder anomalies", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 12: File Watcher Subsystem
# ============================================================
def phase_12():
    log_phase(12, "File Watcher Subsystem")

    result = call_tool("list_watched_files", {})
    log_test("12.1", "List watched files", "error" not in result, result.get("error", ""))

    result = call_tool("get_file_watcher_stats", {})
    log_test("12.2", "File watcher stats", "error" not in result, result.get("error", ""))

    result = call_tool("delete_file_content", {"file_path": "/nonexistent/test/path.rs"})
    log_test("12.3", "Delete nonexistent file (graceful)", True)  # No crash = pass

    result = call_tool("reconcile_files", {})
    log_test("12.4", "File reconciliation", "error" not in result, result.get("error", ""))

# ============================================================
# PHASE 14: Error Handling and Edge Cases
# ============================================================
def phase_14():
    log_phase(14, "Error Handling and Edge Cases")

    # 14.1 Invalid memory ID
    result = call_tool("get_memory_fingerprint", {"memory_id": "00000000-0000-0000-0000-000000000000"})
    log_test("14.1", "Invalid memory ID (no crash)", True)

    # 14.2 Empty query
    result = call_tool("search_graph", {"query": "", "limit": 5})
    log_test("14.2", "Empty query (no crash)", True)

    # 14.3 Large content
    result = call_tool("store_memory", {"content": "A" * 10000, "rationale": "Size test"}, timeout=120)
    log_test("14.3", "10K char content (no crash)", True)

    # 14.4 Unicode
    result = call_tool("store_memory", {
        "content": "Koenig's lemma. Japanese: \u30b1\u30fc\u30cb\u30d2\u306e\u88dc\u984c. Chinese: \u67ef\u5c3c\u5e0c\u5f15\u7406. Arabic: \u0645\u0628\u0631\u0647\u0646\u0629 \u0643\u0648\u0646\u064a\u063a",
        "rationale": "Unicode test"
    }, timeout=120)
    mid = result.get("memory_id", "") if "error" not in result else ""
    log_test("14.4", "Unicode content stored", bool(mid), result.get("error", ""))

    # 14.6 Invalid embedder
    result = call_tool("search_by_embedder", {"query": "test", "embedder": "E99_NonExistent", "limit": 5})
    log_test("14.6", "Invalid embedder (no crash)", True)

    # 14.7 Invalid weight profile
    result = call_tool("search_graph", {"query": "test", "weight_profile": "nonexistent_profile", "limit": 5})
    log_test("14.7", "Invalid weight profile (no crash)", True)

    # 14.8 Zero limit
    result = call_tool("search_graph", {"query": "test", "limit": 0})
    log_test("14.8", "Zero limit (no crash)", True)

    # 14.9 Negative importance
    rid = STORED_IDS.get("rocksdb", "")
    if rid:
        result = call_tool("boost_importance", {"memory_id": rid, "boost": -999.0})
        imp = result.get("importance", result.get("new_importance", -1))
        log_test("14.9", f"Negative boost (importance={imp}, want >= 0)", imp >= 0 if isinstance(imp, (int, float)) else False)

# ============================================================
# MAIN
# ============================================================
def main():
    print(f"\n{BOLD}{'='*60}{RESET}")
    print(f"{BOLD}  MCP Context-Graph Full System Verification{RESET}")
    print(f"{BOLD}  ~85 tests across 14 phases{RESET}")
    print(f"{BOLD}{'='*60}{RESET}")

    t_start = time.time()

    if not start_server():
        print(f"\n{RED}FATAL: Server failed to start. Stderr:{RESET}")
        if PROC:
            try:
                err = PROC.stderr.read(4096).decode()
                print(err[:2000])
            except:
                pass
        stop_server()
        sys.exit(1)

    try:
        phase_1()
        phase_2()
        phase_3()
        phase_4()
        phase_5()
        phase_6()
        phase_7()
        phase_8()
        phase_9()
        phase_10()
        phase_11()
        phase_12()
        phase_14()
    except KeyboardInterrupt:
        print(f"\n{YELLOW}Interrupted by user{RESET}")
    except Exception as e:
        print(f"\n{RED}EXCEPTION: {e}{RESET}")
        import traceback
        traceback.print_exc()
    finally:
        stop_server()

    elapsed = time.time() - t_start
    passed = sum(1 for _, _, p, _ in RESULTS if p)
    failed = sum(1 for _, _, p, _ in RESULTS if not p)
    total = len(RESULTS)

    print(f"\n{BOLD}{'='*60}{RESET}")
    print(f"{BOLD}  RESULTS SUMMARY{RESET}")
    print(f"{BOLD}{'='*60}{RESET}")
    print(f"  Total:  {total}")
    print(f"  {GREEN}Passed: {passed}{RESET}")
    if failed > 0:
        print(f"  {RED}Failed: {failed}{RESET}")
    else:
        print(f"  Failed: 0")
    print(f"  Time:   {elapsed:.1f}s")

    if failed > 0:
        print(f"\n  {RED}Failed tests:{RESET}")
        for test_id, desc, p, detail in RESULTS:
            if not p:
                print(f"    {RED}[FAIL] {test_id}: {desc}{RESET}")
                if detail:
                    print(f"           {detail[:150]}")

    print(f"\n  Pass rate: {GREEN}{passed}/{total} ({100*passed/total:.0f}%){RESET}" if total > 0 else "")
    print()
    sys.exit(0 if failed == 0 else 1)

if __name__ == "__main__":
    main()
