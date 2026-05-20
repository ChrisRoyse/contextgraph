# syntax=docker/dockerfile:1.7

ARG CUDA_IMAGE=docker.io/nvidia/cuda:13.2.0-devel-ubuntu24.04@sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d
FROM ${CUDA_IMAGE}

LABEL org.opencontainers.image.title="contextgraph-mejepa"
LABEL org.opencontainers.image.description="Local DynamicJEPA reproduction image for the RTX 5090 O*NET paper-small artifact"
LABEL org.opencontainers.image.source="https://local/contextgraph"

ENV CUDA_COMPUTE_CAP=120
ENV CONTEXT_GRAPH_ENV=development
ENV CONTEXT_GRAPH_BIN=/usr/local/bin/context-graph
ENV ONET_TEXT_ZIP=/data/onet/db_30_2_text.zip
ENV SYSTEM_SPECS=/workspace/docs2/aiwonder.md
ENV FIXTURES_ROOT=/workspace/configs/dynamicjepa
ENV MEJEPA_REPRO_RUN_ROOT=/work/mejepa_reproduce_runs

WORKDIR /workspace

COPY --chmod=0755 target/debug/context-graph /usr/local/bin/context-graph
COPY --chmod=0755 reproduce.sh /workspace/reproduce.sh
COPY configs/dynamicjepa /workspace/configs/dynamicjepa
COPY reference /workspace/reference
COPY docs2/aiwonder.md /workspace/docs2/aiwonder.md

ENTRYPOINT ["/workspace/reproduce.sh"]
CMD ["--help"]
