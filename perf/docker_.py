"""Docker container lifecycle management for benchmark comparisons."""

from __future__ import annotations

import logging
import shutil
import subprocess
import sys
import time

log = logging.getLogger("perf.docker")


def find_docker() -> str:
    """Return path to docker CLI, or exit if not found."""
    docker = shutil.which("docker")
    if not docker:
        log.error("Docker not found in PATH. Install Docker and try again.")
        sys.exit(1)
    return docker


def start_container(
    docker: str,
    image: str,
    name: str,
    port: int,
    cpus: str,
    memory: str,
    pids_limit: str | int,
) -> None:
    """Start a Redis Docker container with resource limits."""
    cmd = [
        docker, "run", "--rm", "-d",
        "--name", name,
        "-p", f"{port}:6379",
        "--cpus", str(cpus),
        "--memory", str(memory),
        "--memory-swap", str(memory),
        "--pids-limit", str(pids_limit),
        "--ulimit", "nofile=65535:65535",
        "-e", "REDIS_BIND=0.0.0.0",
        "-e", "REDIS_PORT=6379",
        image,
    ]
    log.info("Starting container %s from image '%s' ...", name, image)
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        log.error("Failed to start container %s:\n%s", name, result.stderr.strip())
        sys.exit(1)
    log.info("Container %s is running.", name)


def stop_container(docker: str, name: str) -> None:
    """Stop a Docker container (errors ignored)."""
    log.info("Stopping container %s ...", name)
    subprocess.run([docker, "stop", name], capture_output=True, text=True)


def wait_for_ready(port: int, wait_secs: float = 2.0) -> None:
    """Simple wait for Redis to be ready after container start."""
    log.info("Waiting %.1f s for Redis to be ready ...", wait_secs)
    time.sleep(wait_secs)
