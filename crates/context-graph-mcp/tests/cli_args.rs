use std::process::{Command, Output};

fn run_mcp(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_context-graph-mcp"));
    command
        .args(args)
        .env_remove("CONTEXT_GRAPH_TRANSPORT")
        .env_remove("CONTEXT_GRAPH_TCP_PORT")
        .env_remove("CONTEXT_GRAPH_HTTP_PORT")
        .env_remove("CONTEXT_GRAPH_BIND_ADDRESS")
        .env_remove("CONTEXT_GRAPH_DAEMON_PORT")
        .env_remove("CONTEXT_GRAPH_DAEMON")
        .env_remove("CONTEXTGRAPH_DATA_ROOT")
        .env_remove("CONTEXTGRAPH_LOCAL_SMOKE_ISSUE");
    command.output().expect("run context-graph-mcp binary")
}

#[test]
fn cli_daemon_rejects_local_d_root_in_production_mode() {
    let output = run_mcp(&[
        "--daemon-toolchain-audit-only",
        "--d-root",
        "/zfs/archive/contextgraph",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "production daemon root must reject local aiwonder"
    );
    assert!(
        stderr.contains("MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER"),
        "stderr must include production-root guard code; got: {stderr}"
    );
}

#[test]
fn cli_rejects_zero_ports_before_startup() {
    for args in [
        ["--port", "0"],
        ["--daemon-port", "0"],
        ["--http-port", "0"],
    ] {
        let output = run_mcp(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !output.status.success(),
            "zero port args must fail closed: {args:?}"
        );
        assert!(
            stderr.contains("1-65535"),
            "stderr must explain valid port bounds for {args:?}; got: {stderr}"
        );
    }
}

#[test]
fn cli_rejects_unknown_argument_before_startup() {
    let output = run_mcp(&["--mystery"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "unknown args must fail closed");
    assert!(
        stderr.contains("Unknown argument '--mystery'"),
        "stderr must identify the unknown argument; got: {stderr}"
    );
}

#[test]
fn cli_rejects_missing_argument_values_before_startup() {
    for args in [
        ["--config"],
        ["--transport"],
        ["--mode"],
        ["--port"],
        ["--daemon-port"],
        ["--http-port"],
    ] {
        let output = run_mcp(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !output.status.success(),
            "missing CLI value must fail closed: {args:?}"
        );
        assert!(
            stderr.contains("Missing value"),
            "stderr must explain the missing value for {args:?}; got: {stderr}"
        );
    }
}
