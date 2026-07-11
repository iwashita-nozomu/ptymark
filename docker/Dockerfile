# @dependency-start
# contract environment
# responsibility Defines the canonical development container image.
# upstream environment README.md docker runtime guidance
# downstream implementation install_python_dependencies.sh installs Python modules after workspace mount
# downstream implementation ../tools/ci/check_fresh_clone.sh uses rsync for workspace overlay
# downstream implementation ../tools/docker_dependency_validator.sh validates canonical image tools
# @dependency-end
FROM nvidia/cuda:12.2.0-devel-ubuntu22.04
ENV DEBIAN_FRONTEND=noninteractive

# GCCやビルド系 + Python 3.11
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git docker.io \
    build-essential pkg-config cmake ninja-build \
    python3.11 python3.11-dev python3.11-venv python3-venv python3-pip \
    openssh-client graphviz rsync \
 && update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.11 20 \
 && python3 --version | grep -E '^Python 3\.11\.' \
 && rm -rf /var/lib/apt/lists/*

COPY docker/register_safe_directories.sh /usr/local/bin/project-template-register-safe-directories
RUN chmod +x /usr/local/bin/project-template-register-safe-directories

WORKDIR /workspace
ENV PYTHONPATH=/workspace/python
ENV CMAKE_GENERATOR=Ninja
ENV PROJECT_TEMPLATE_CPP_BUILD_ROOT=/workspace/build/cpp
ENV PROJECT_TEMPLATE_CPP_INSTALL_ROOT=/workspace/.state/cpp-install

EXPOSE 8888

# Canonical image 側では workspace を safe.directory に固定する。
# mount 済み workspace では同じ helper が vendor 直下の directory も動的に登録する。
RUN set -eux; \
    project-template-register-safe-directories /workspace

# docker CLI は同梱するが、build/run をコンテナ内から行うときは
# host socket の mount か別 daemon が必要。
RUN ssh -V
RUN docker --version

CMD ["/bin/bash"]
