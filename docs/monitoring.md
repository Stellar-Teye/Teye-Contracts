# Stellar-Teye Contract Monitoring System

This document outlines the monitoring infrastructure for the Stellar-Teye smart contracts, including setup instructions, metrics collection, and alerting runbooks.

## Architecture

The monitoring stack is built on a standard Docker-compose configuration:
- **Prometheus**: Metrics scraping and long-term time-series storage.
- **Alertmanager**: Routing alerts triggered by Prometheus rules.
- **Grafana**: Visualizing metrics and contract health.
- **Health Check Script**: A bash script (`health_check.sh`) designed to periodically ping the Soroban RPC and contract to verify liveness.

## Setup Instructions

1. Ensure you have Docker and Docker Compose installed.
2. Navigate to the monitor directory:
   ```bash
   cd scripts/monitor
   ```
3. Start the monitoring stack in the background:
   ```bash
   docker-compose up -d
   ```
4. Access the services:
   - **Grafana**: [http://localhost:3000](http://localhost:3000) (default credentials: `admin` / `admin`)
   - **Prometheus**: [http://localhost:9090](http://localhost:9090)
   - **Alertmanager**: [http://localhost:9093](http://localhost:9093)

Grafana is pre-provisioned with the Prometheus datasource and a **Contract Health Dashboard**.

## Adding Custom Metrics

To track new contract metrics:
1. Ensure your exporter application or health script pushes metrics (e.g., via Prometheus Pushgateway) or exposes a `/metrics` endpoint.
2. Update `scripts/monitor/prometheus/prometheus.yml` to scrape the new endpoint.
3. Update or create new Dashboards in `scripts/monitor/grafana/dashboards`.

## Alerting Runbooks

When Prometheus triggers an alert, Alertmanager routes it based on its severity. Below are the runbooks for the currently configured alerts.

### 🚨 Alert: `RPCDown`
- **Severity**: Critical
- **Description**: The Soroban RPC service or health exporter has been unreachable for > 1 minute.
- **Impact**: Applications cannot read from or deploy to the Stellar network. Users may experience total service failure.
- **Action Plan**:
  1. Check the availability of the RPC URL (default `https://rpc-futurenet.stellar.org`).
  2. Verify your network connection and DNS resolution.
  3. If using a local/private RPC node, check its docker container or systemctl status and its logs for `out of memory` or sync errors.

### ⚠️ Alert: `HighErrorRate`
- **Severity**: Warning
- **Description**: More than 5 transaction errors per second over the last 5 minutes.
- **Impact**: Users are likely failing to execute smart contract calls (e.g. invalid arguments, lacking authorization, or network congestion).
- **Action Plan**:
  1. Open the **Contract Health Dashboard** in Grafana to see the error spikes.
  2. Check recent contract deployments or DApp frontend updates that might have introduced breaking changes.
  3. Look at the Soroban CLI logs or application backend logs for specific transaction failure codes (e.g., `HostError`).

### ⚠️ Alert: `SlowRPCResponse`
- **Severity**: Warning
- **Description**: Contract RPC calls are taking longer than 2.0 seconds on average.
- **Impact**: Degraded user experience. DApp may feel laggy.
- **Action Plan**:
  1. Check the Soroban RPC load. If using a public RPC, consider migrating to a dedicated node.
  2. If using a private node, check host CPU, Memory, and Disk I/O (Stellar nodes can be IOPS-heavy).
